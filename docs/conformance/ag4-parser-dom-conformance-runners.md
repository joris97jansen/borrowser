# AG4 parser and DOM conformance runners

AG4 adds a test/tooling execution layer for the three HTML/parser observation
surfaces already named by AG2. It does not move parser-fixture semantics into
generic AG support and does not add rendering or runtime execution.

## Ownership and dependency direction

The dependency direction is:

```text
conformance-runner
  |-- conformance-test-support
  `-- html-test-support
        `-- html::conformance
              `-- production HTML parser
```

`conformance-test-support` owns generic inventory, classification, and
eligibility. It has no HTML, CSS, Layout, Paint/GFX, Browser/runtime, or
JavaScript dependency. `html-test-support` owns the strict parser-fixture
schema, validation, execution plan, comparison, disposition compatibility,
and canonical snapshot codecs. Neither dependency knows about the AG execution
orchestration crate.

The orchestration crate has no adapter enabled by default. Its optional
`html-parser` feature activates `html-test-support` explicitly. A semantic
Cargo-metadata guard resolves direct and workspace-inherited dependency aliases,
includes target-specific declarations, and checks the default and HTML-enabled
workspace feature closures independently; it does not rely on dependency-key
text matching.

## Canonical evaluation boundary

AG initiates exactly one canonical AE fixture evaluation for an attempted
case. One evaluation is not one production parser invocation: AE retains its
complete validated baseline and parity schedule and may invoke the parser zero,
one, or multiple times. AG never starts a second evaluation to recover output
after mismatch, incompleteness, or execution failure.

AE fixture V1 keeps its legacy single-delivery regression behavior. AG may
load and inspect V1, but AG4 executable profiles require the validated modern
canonical-observation parity execution model used by fixture V2 and V3. The
HTML adapter checks a semantic execution-model view derived from the validated
execution plan; it does not inspect the serialized format string.

AG-owned nested AE fixtures use active disposition. AE disposition remains a
parser-fixture compatibility policy, while AG3 remains the sole authority for
AG expectation, expected-failure, and classification facts.

## AG package boundary

AG fixture V2 contains one opaque subsystem-owned execution package. The AG
descriptor declares the nested entry point and every support file explicitly.
There is no unrestricted assets directory and no package-kind taxonomy. V1
continues to reject nested fixture descriptors. V2 permits only its exact
declared nested entry and rejects every undeclared regular file.

The HTML adapter points the existing canonical AE loader at the declared
package root. It never constructs `ValidatedFixtureSpec`, parses the nested
TOML, reads expectation sidecars independently, validates hashes, or
reimplements delivery planning.

Manifest V2 records `fixture_format` for every case and, for V2 packages, the
validated `execution_entry_path` and sorted `execution_support_paths`. This is
sufficient review truth for the default-deny package without duplicating AE
parser semantics in the generic manifest.

## Observation profiles

### `html-tokenizer`

The target is the standalone tokenizer. Tokens and exact typed parse errors
are required expectations. Document mode, tree, patches, and transitions are
not applicable. Implementation diagnostics, unsupported features, and final
invariants remain canonical reportable surfaces. Initial native tokenizer
fixtures do not declare empty unsupported-feature expectations merely to prove
absence, and AG invents no tokenizer unsupported identity.

### `html-tree-construction`

The target is the document parser with scripting disabled. Canonical
`html5-dom-v3` tree, exact typed parse errors, and document mode are required
expectations. Patches, transitions, implementation diagnostics, unsupported
features, and final invariants may provide additional explicitly declared
construction evidence. Tokens are not part of this profile's success
predicate.

### `dom-tree`

The target is the document parser with scripting disabled. The final
`html5-dom-v3` parser-created DOM is the expected semantic subject. Parse
errors, document mode, diagnostics, unsupported features, and final-invariant
state remain auxiliary actual evidence and are never inferred from the DOM.
Construction patches and transitions are not part of the DOM success
predicate. The initial AG4 DOM package contract permits exactly one declared
expectation surface: `tree`. Declared token, parse-error (exact or count),
document-mode, patch, transition, implementation-diagnostic,
unsupported-feature, and final-invariant expectations are rejected during
profile reconciliation before execution. A real AE final-invariant execution
failure can still invalidate a run; that does not make a final-invariant
sidecar part of DOM equivalence.

Native AG4 tokenizer and tree-construction fixtures require exact parse-error
expectations. AE V3 count expectations remain distinguishable typed metadata;
AG does not synthesize exact identities from a count. Count strength therefore
cannot satisfy the tokenizer or tree-construction exact-error requirement and
is not a permitted DOM expectation.

## Orthogonal result semantics

AG keeps classification completeness, engine capability, harness readiness,
environment eligibility, expectation, stability, attempt state, observed
outcome, canonical observations, and derived policy separate. In particular:

- unavailable capability means not runnable, not attempted, and no parser
  outcome;
- a captured parser unsupported-feature observation can coexist with an
  attempted semantic pass or mismatch and does not change AG capability facts;
- XFAIL and XPASS are derived from AG3 expectation plus the observed semantic
  outcome, never from AE's policy-level `Pass`;
- configured parser resource diagnostics, incomplete observation, fatal
  parser/observation resource failure, and AG report failure remain distinct.

The normalized Rust model enforces the attempt/outcome relation structurally:
`ExecutionAttempt::Attempted` contains exactly one terminal
`ObservedExecutionOutcome`, while `ExecutionAttempt::NotAttempted` contains no
observed execution outcome. Useful AE results produced before the production
observation executor—such as unsupported fixture semantics, unsupported
expectations, or AE non-execution—are retained as typed pre-attempt evaluation
information instead.

AG3 state is evaluated before execution infrastructure is required.
Not-yet-classified and harness-not-ready/not-yet-established parser cases are
reported without loading a subsystem package. A `ready` harness assertion is
stronger: its package must load and reconcile even when an unavailable engine
capability makes execution ineligible. Only a runnable case with a successfully
reconciled package reaches the single AG-initiated AE evaluation.

Closed requirement, capability, harness-limitation, environment-requirement,
expected-failure, lane-policy, parser-surface, execution-failure,
incompleteness, and AE-disposition values remain Rust enums through
normalization. Only the report codec spells them as text. Human reasons,
validated opaque feature/profile IDs, delivery identities, detailed stable AE
failure identities, and mismatch text intentionally remain strings.

## Deterministic bounded reporting

`borrowser-conformance-parser-report-v1` is a closed canonical framed-text
format with fixed field/section order and explicit `BEGIN`/`END` case and
artifact records; it is not TOML and Rust `Debug` is not its codec. It is
constructed completely in a bounded fallible buffer before the first stdout
write. Optional scalar absence is the reserved unquoted token `null`; present
strings are always quoted and escaped, so absence differs from both `"none"`
and `"null"`, and absent numeric values differ from zero. The fixed versioned
report limits are selected during AG4
from measured focused-corpus snapshots, deterministic encoding overhead,
existing AE guardrails, and practical CI headroom. Fixtures, hosts,
environment variables, and CLI arguments cannot change them. Reports and
diagnostics are never silently truncated.

A construction, allocation, size, or snapshot-serialization failure publishes
no report bytes. Once publication begins, stdout is ordinary stream I/O: an
output failure is a distinct transport failure and a prefix may already have
been accepted. AG does not claim stdout atomicity.

These AG report limits do not complete AE's deferred byte-bounded parser
observation payload accounting. AE event/unit guardrails and fallible
allocation semantics remain authoritative at their existing boundary.

The fixed V1 limits are 8 MiB per embedded canonical observation, 1 MiB per
mismatch diagnostic, and 32 MiB for the complete report. At introduction, the
largest reviewed AE canonical sidecar was 2,971 bytes and all AG4 seed
artifacts were smaller; the limits provide more than 2,700x and 350x those
measured sizes respectively, plus deterministic escaping and multi-artifact CI
headroom. The complete seven-case report measured 30,269 bytes including its
fixed framing and escaping, so the total bound also retains substantial CI
headroom. AE capture guardrails range from 65,536 to 262,144 retained
events/units and remain event-count, not byte-memory, bounds. Exact-boundary
tests enforce the AG limits; excess or allocation failure is a run-level
harness/reporting failure with no stdout bytes published and no truncation.

Serialized observation evidence is checked against the 8 MiB artifact bound
immediately when it crosses from AE into AG, before retention. Mismatch text is
likewise checked before cloning. The aggregate bytes of AG-retained observation
and mismatch evidence are limited to 32 MiB across the run with checked
arithmetic; rejection is non-mutating. Report construction then applies the
same per-evidence checks defensively and independently limits the complete
encoded report to 32 MiB. Thus AG owns at most 32 MiB of retained evidence plus
one at-most-32-MiB publication buffer (and bounded case metadata), while AE's
temporary complete snapshot serialization allocation remains its explicitly
deferred accounting issue.

Use `make check-conformance-parser` for the CI-safe seven-case corpus. The CLI
writes the deterministic report to stdout when invoked with the explicit
adapter feature: `cargo run -p conformance-runner --features html-parser
--locked -- --check`. The subsystem-neutral crate has no adapter enabled by
default.
`--check` exits non-zero for an unexpected
policy result; report construction/serialization failures exit separately from
stdout transport failures.

## Focused seed corpus and oracle discipline

AG4 has two tokenizer cases, three tree-construction cases, and two DOM cases.
The runnable inputs and snapshots are byte-for-byte adaptations of reviewed AE
fixtures or small combinations whose empty error/mode facts follow directly
from the HTML parsing rules. The malformed tokenizer and active-formatting
cases retain reviewed exact error identities. The representative static DOM is
the reviewed AE fixture. The standards-derived repeated-body expectation is
retained but not executed while body-attribute merging is unavailable. Its
declared tree observes that gap; its exact parse errors, document mode, and tree
do not observe the independently tracked repeated-body `frameset_ok` gap. No
snapshot was created by blessing an AG run, and no repository XFAIL was
invented.

## Scope

AG4 is a focused in-repository harness. It does not claim broad WPT or html5lib
compliance, complete HTML conformance, JavaScript support, general browser
compatibility, CSS/Layout/Paint execution, reftest or pixel infrastructure, or
cross-engine execution.
