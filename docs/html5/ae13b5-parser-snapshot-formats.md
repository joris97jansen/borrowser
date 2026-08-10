# AE13b5 Canonical Parser Snapshot Formats

## Ownership

`html::conformance::execute_parser_observation` and its typed
`CanonicalParserResult` are the only semantic source. `html-test-support` owns
the textual codecs, sidecar comparison, and diagnostics. Writers never consume
legacy `TokenFmt`, legacy DOM/patch serializers, raw parser IDs, `PatchKey`,
materialized IDs, runtime patch batches, memory addresses, hash-map iteration,
platform path syntax, descriptions, or Rust `Debug`.

Readers validate versioned grammar and format-local framing. They do not decide
whether the parser should emit a token, namespace, mode, tree topology, patch,
transition, or unsupported feature.

## Shared physical and lexical grammar

Every canonical snapshot is valid UTF-8 without BOM. It uses LF only, has its
exact header on physical line one, and ends with one terminal LF. CR, CRLF,
comments, blank lines, duplicate headers, leading/trailing whitespace, double
field spaces, and trailing tokens are illegal. Each semantic record occupies
exactly one physical line.

Fields have fixed order. Unsigned integers use canonical base-10 with no sign
or leading zero except `0`. Booleans are `true` or `false`. Optional strings are
unquoted `null` or a quoted string; therefore `null` and `"null"` differ.

Quoted strings preserve UTF-8 scalar values. Canonical escapes are `\"`,
`\\`, `\n`, `\r`, `\t`, and uppercase fixed four-digit `\uXXXX` for remaining
C0 controls and DEL. Raw control characters, unknown escapes, noncanonical
alternate escapes, and unpaired scalar encodings are malformed.

Malformed headers, unknown versions, unknown closed spellings, missing or
duplicate fields, invalid escapes, invalid locations, duplicate locations,
noncontiguous local indices, and trailing content are `SnapshotFormat(surface)`.
Future incompatible grammar receives a new format identifier.

## Inventory and empty forms

| Surface | Exact header | Empty/header-only legal |
|---|---|---|
| tokens | `# format: html5-token-v2` | no; one final EOF required |
| parse errors | `# format: html5-parse-errors-v1` | yes |
| implementation diagnostics | `# format: html5-implementation-diagnostics-v1` | yes |
| document mode | `# format: html5-document-mode-v1` | no; exactly one record |
| canonical tree | `# format: html5-dom-v3` | yes |
| canonical patch stream | `# format: html5-dompatch-v3` | yes |
| transitions | `# format: html5-tree-transitions-v1` | yes |
| unsupported features | `# format: html5-unsupported-features-v1` | yes |

An empty requested collection is its header-only snapshot. An absent
expectation is not read, requested, serialized, or compared.

The per-format framing rules are:

| Surface | Record ordinal/local index | Duplicate location | Content after the final allowed record |
|---|---|---|---|
| tokens | token ordinals start at 1 and are contiguous; attributes start at 0 for each start tag and are contiguous | rejected | rejected; EOF must be the sole final token record |
| parse errors | occurrences start at 1 and are contiguous | rejected by occurrence framing | rejected as an unknown additional record |
| implementation diagnostics | occurrences start at 1 and are contiguous | rejected by occurrence framing | rejected as an unknown additional record |
| document mode | no ordinal; exactly one `MODE` location | a second mode is rejected | rejected |
| canonical tree | root indices start at 0; child indices start at 0 for each parent; attribute indices start at 0 for each owner; all are contiguous | rejected for node, boundary, and attribute locations | rejected as an unknown additional record |
| canonical patches | operation ordinals start at 1 and are contiguous; attributes start at 0 for each owning operation and are contiguous | rejected | rejected as an unknown additional record |
| transitions | occurrences start at 1 and are contiguous | rejected by occurrence framing | rejected as an unknown additional record |
| unsupported features | occurrences start at 1 and are contiguous | rejected by occurrence framing | rejected as an unknown additional record |

There is no separate trailer syntax in any format. Header-only tree and patch
snapshots are grammatically legal even though a successful document execution
normally emits structural records and patch operations.

## Tokens: html5-token-v2

Token records use contiguous one-based ordinals:

```text
TOKEN ordinal=<n> kind=doctype name=<optional-string> public-id=<optional-string> system-id=<optional-string> force-quirks=<bool>
TOKEN ordinal=<n> kind=start-tag name=<string> self-closing=<bool>
TOKEN ordinal=<n> kind=end-tag name=<string>
TOKEN ordinal=<n> kind=character data=<string>
TOKEN ordinal=<n> kind=comment data=<string>
TOKEN ordinal=<n> kind=processing-instruction target=<string> data=<string>
TOKEN ordinal=<n> kind=eof
```

Start-tag attributes immediately follow their owning token and use contiguous
zero-based local indices:

```text
TOKEN_ATTRIBUTE token=<token-ordinal> index=<n> name=<string> value=<string>
```

There is exactly one EOF and it is the final record. Semantic names are quoted;
v1 bare-name and sentinel restrictions do not apply. V1 remains compatibility
only because `DOCTYPE name=null` cannot distinguish absence from the literal
name `null`. An incompatible token change requires `html5-token-v3`.

## Parse errors and implementation diagnostics

Parse errors use contiguous one-based occurrences and the exact form:

```text
PARSE_ERROR occurrence=<n> stage=<closed-stage> code=<closed-code> recovery=<closed-recovery-or-null> position=<position> context=<absent|present> context-token=<closed-or-null> context-mode=<closed-or-null> context-namespace=<closed-or-null>
```

Implementation diagnostics use:

```text
IMPLEMENTATION_DIAGNOSTIC occurrence=<n> stage=<closed-stage> code=<closed-code> payload=<code-owned-payload> position=<position> context=<absent|present> context-token=<closed-or-null> context-mode=<closed-or-null> context-namespace=<closed-or-null>
```

The closed spellings are exhaustive matches over the current typed enums.
Descriptions are forbidden because their wording is not semantic identity.
Known positions encode normalized UTF-8 offset, one-based line/column, and
typed source provenance; unavailable positions retain their exact reason.
Context presence is explicit, so absent context differs from a present context
whose optional fields are all null.

## Document mode

Exactly one record follows the header:

```text
MODE value=<no-quirks|limited-quirks|quirks>
```

The reader validates only the closed spelling. It does not reclassify the
doctype.

## Canonical tree: html5-dom-v3

Structural records are depth-first in canonical vector/traversal order:

```text
NODE path=<tree-path> kind=document
NODE path=<tree-path> kind=document-type name=<optional-string> public-id=<optional-string> system-id=<optional-string>
NODE path=<tree-path> kind=element namespace=<html|svg|mathml> local-name=<string>
NODE path=<tree-path> kind=html-template-host
NODE path=<tree-path> kind=text data=<string>
NODE path=<tree-path> kind=comment data=<string>
NODE path=<tree-path> kind=processing-instruction target=<string> data=<string>
```

Attributes immediately follow their element or template host:

```text
ATTRIBUTE path=<owner-path> index=<zero-based-local-index> namespace=<none|xml|xmlns|xlink> prefix=<optional-string> local-name=<string> value=<string>
```

Tree-path lexical grammar is `/root[<canonical-u64>]` followed by zero or more
complete `/child[<canonical-u64>]` or `/contents` segments. A node or host path
cannot end in `/contents`; the explicit boundary form can. Multiple `/contents`
segments are legal when templates are nested, for example
`/root[0]/contents/child[0]/contents`. Malformed segment prefixes, suffixes,
signs, and noncanonical decimal indices are rejected lexically without deciding
whether any segment names a real template host. A template contents boundary is
explicit:

```text
TEMPLATE_CONTENTS path=<host-path>/contents host=<host-path>
```

Its children use `<host-path>/contents/child[n]`. The writer uses an explicit
work stack and never recursively descends the canonical tree. It emits
canonical preorder: the node, its attributes, then ordinary children in vector
order; an HTML template host then emits its one contents boundary followed by
contents children in vector order. The serializer therefore introduces no
native-call-stack limit below the canonical tree-unit guardrail.

Lexical path validation is independent from the iterative traversal/framing
state machine. Framing rejects a
child before its parent, an attribute anywhere except immediately after its
owner, contents children before the boundary, ordinary template children after
the boundary, non-preorder sibling or ancestor transitions, duplicate or
skipped root/child/attribute indices, duplicate locations, a repeated contents
boundary for one host, and a template host whose boundary never appears. Each
nested template host has an independent frame and exactly one boundary. The
reader validates only serialized traversal framing: semantically implausible element names,
namespaces, and otherwise framed topology remain legal snapshot data. It does
not reconstruct a DOM or judge parser behavior. Attributes and the
`ObservedTree` wrapper are records but not production canonical tree-capacity
units.

## Canonical patches: html5-dompatch-v3

Patch records use contiguous one-based operation ordinals and one exact shape
for each `ObservedPatchOperation` variant:

```text
PATCH operation=<n> kind=<closed-operation> <fixed operation fields>
```

`create-element` and `set-attributes` may be followed by contiguous zero-based:

```text
PATCH_ATTRIBUTE operation=<n> index=<n> namespace=<closed> prefix=<optional-string> local-name=<string> value=<string>
```

Labels and operation order come only from AE13b3 canonical projection. Every
quoted node label has the exact form `node-<positive-canonical-decimal>`:
`node-1` is valid, while `node-0`, `node-01`, an empty label, an unquoted label,
or another prefix is malformed. Every operation ordinal and patch-attribute
index is validated as a canonical unsigned decimal before conversion; signs
and leading zeroes are forbidden. Readers validate operation framing and
attribute grouping, not live-DOM applicability, creation history, or semantic
patch ordering.

The complete patch-record inventory and fixed field order is:

```text
PATCH operation=<n> kind=clear
PATCH operation=<n> kind=create-document node=<string> legacy-doctype=<optional-string>
PATCH operation=<n> kind=create-document-type node=<string> name=<optional-string> public-id=<optional-string> system-id=<optional-string>
PATCH operation=<n> kind=create-element node=<string> namespace=<html|svg|mathml> local-name=<string>
PATCH operation=<n> kind=create-template-contents host=<string> contents=<string>
PATCH operation=<n> kind=create-text node=<string> text=<string>
PATCH operation=<n> kind=create-comment node=<string> data=<string>
PATCH operation=<n> kind=create-processing-instruction node=<string> target=<string> data=<string>
PATCH operation=<n> kind=append-child parent=<string> child=<string>
PATCH operation=<n> kind=insert-before parent=<string> child=<string> before=<string>
PATCH operation=<n> kind=remove-node node=<string>
PATCH operation=<n> kind=set-attributes node=<string>
PATCH operation=<n> kind=set-text node=<string> text=<string>
PATCH operation=<n> kind=append-text node=<string> text=<string>
```

Only `create-element` and `set-attributes` may own `PATCH_ATTRIBUTE` records.
An empty owned attribute group is legal. A patch attribute under any other
operation, before its operation, after another operation, with a different
operation ordinal, or with a noncontiguous local index is malformed framing.

## Transitions and unsupported features

Transitions use contiguous occurrences:

```text
TRANSITION occurrence=<n> token-kind=<closed> token-name=<optional-string> token-data=<optional-string> token-self-closing=<bool-or-null> mode-before=<closed> dispatch=<closed> mode-after=<closed> reprocessed=<bool>
```

Unsupported features use:

```text
UNSUPPORTED_FEATURE occurrence=<n> subsystem=tree-construction feature=<closed-feature> context-token=<closed-or-null> context-mode=<closed-or-null> context-namespace=<closed-or-null>
```

Ordering and identities come only from AE13b4 production observations. Readers
do not reproduce dispatch selection or unsupported-feature eligibility.

Closed token spellings are `doctype`, `start-tag`, `end-tag`, `character`,
`comment`, `processing-instruction`, and `eof`. Closed insertion modes are
`initial`, `before-html`, `before-head`, `in-head`, `after-head`, `in-body`,
`after-body`, `after-after-body`, `in-table`, `in-table-text`, `in-caption`,
`in-column-group`, `in-table-body`, `in-row`, `in-cell`, `in-template`, and
`text`. Transition dispatch is `html-insertion-mode:<closed-mode>`,
`shared-template-rules`, `foreign-content`, or `text-mode`. Unsupported feature
spellings are `merge-attributes-into-existing-html-element`,
`merge-attributes-into-existing-body-element`,
`mark-frameset-not-ok-for-repeated-body-start-tag`,
`require-same-named-table-cell-in-scope-for-end-tag`,
`generate-implied-end-tags-and-check-current-node-before-closing-table-cell`,
and `generate-implied-end-tags-and-check-current-node-before-closing-caption`.

Closed parse-error and implementation-diagnostic codes retain their explicit
family prefixes (`standard:`, `tokenizer-extension:`, `tree-construction:`,
`parser-resource-limit:`, `parser-guardrail:`, or the exact UTF-8 replacement
identity). Their codec matches are exhaustive over the typed production enums;
there is no generic string, description, `Debug`, or unknown-code branch.

## Diagnostics

Mismatch diagnostics name fixture ID, normalized repository-relative fixture
path, stable surface, transition delivery where applicable, expected sidecar,
format, first differing record and semantic location, expected/actual line,
nearby context, and both record counts. Surface and format are sealed together
in typed parsed/canonical snapshot variants rather than independently supplied.

Each codec owns distinct parsed and canonical newtypes with private
constructors. Outer variants accept only the matching surface type, so a token
codec cannot construct a patch snapshot. Accessors derive surface and format
exhaustively from the variant. Snapshot storage retains one UTF-8 backing
string plus semantic locations and byte ranges for records; it does not retain
another owned copy of every record line.

## AE13c final invariants: `html5-final-invariants-v1`

The header is exact:

```text
# format: html5-final-invariants-v1
```

Exactly 16 records follow. They use contiguous one-based ordinals and this
fixed field order:

```text
INVARIANT ordinal=1 field=decoder-carry-empty outcome=<outcome>
INVARIANT ordinal=2 field=preprocessing-flushed outcome=<outcome>
INVARIANT ordinal=3 field=eof-emitted-once outcome=<outcome>
INVARIANT ordinal=4 field=pending-constructs-flushed outcome=<outcome>
INVARIANT ordinal=5 field=output-accounted-for outcome=<outcome>
INVARIANT ordinal=6 field=pending-table-text-empty outcome=<outcome>
INVARIANT ordinal=7 field=insertion-mode-valid outcome=<outcome>
INVARIANT ordinal=8 field=open-elements-consistent outcome=<outcome>
INVARIANT ordinal=9 field=active-formatting-consistent outcome=<outcome>
INVARIANT ordinal=10 field=template-modes-consistent outcome=<outcome>
INVARIANT ordinal=11 field=form-pointer-valid outcome=<outcome>
INVARIANT ordinal=12 field=parent-child-links-valid outcome=<outcome>
INVARIANT ordinal=13 field=namespaces-valid outcome=<outcome>
INVARIANT ordinal=14 field=template-associations-valid outcome=<outcome>
INVARIANT ordinal=15 field=all-patches-materialized outcome=<outcome>
INVARIANT ordinal=16 field=live-tree-matches-materialized-dom outcome=<outcome>
```

Closed outcome spellings are `satisfied`, `failed`,
`not-applicable:standalone-tokenizer-run`, `not-applicable:document-parser-run`, and
`not-applicable:fragment-parser-run`. The reader rejects a missing, extra,
reordered, duplicated, unknown, or noncanonical record. It validates report
framing and stable spellings; it does not inspect parser state or infer whether
an outcome should be satisfied.

AE13c compares typed final-invariant values before writing this representation;
the codec is an external snapshot format, not a parity-equality mechanism.

The writer consumes the fixed `ParserFinalizationReport::fields()` iterator.
Failure inspection uses the report's allocation-free ordered iterator and does
not allocate a temporary vector of failed codes for disposition or snapshot
selection. Rust `Debug` is never serialized.

AE13e reuses these existing snapshot codecs for external WPT-derived
expectations, including html5-dom-v3. Fixture-v3 does not create a snapshot
codec version. External parse-error counts are inline fixture declaration
values rather than a parser-output snapshot format.
