# Conformance fixture inventory

This directory contains the repository-owned AG conformance fixture inventory.
Each logical fixture is a directory bundle containing exactly one authoritative
outer AG `fixture.toml` descriptor plus only its explicitly declared files.
V1 declares one payload and an optional implied-match reference. V2 may
additionally declare one opaque, default-deny subsystem execution package with
one entry and a bounded list of support files. V3 requires an executable
package plus an outer reference kind, match/mismatch relation, and path. V4
adds a lossless `external-derived` lineage without changing V1–V3. See
[`docs/conformance/ag2-fixture-inventory-manifest-contract.md`](../../docs/conformance/ag2-fixture-inventory-manifest-contract.md)
for the normative layout, schema, identity, validation, and manifest contract.
Expected-result and classification metadata lives separately in the
human-authored `expected-results.toml`; see
[`docs/conformance/ag3-expected-results-classification-contract.md`](../../docs/conformance/ag3-expected-results-classification-contract.md).

To add a fixture:

1. Create a bundle directory below `tests/conformance/fixtures/`. Directory
   grouping is for contributors only; it does not define fixture semantics.
   Every path component must follow the contract's lowercase ASCII portable
   component grammar.
2. Add a strict versioned AG `fixture.toml` with a new,
   stable logical ID and explicit scope, observation surface, source kind, and
   bundle-relative payload path.
3. For inventory-only fixtures, use V1 and add only `test_path` and optional
   `[reference].path`. For an executable subsystem package, use V2, keep the
   package nested, declare its exact entry plus every support file, and add no
   undeclared file. The outer AG schema never encodes subsystem semantics.
4. Run `make update-conformance-manifest`.
5. Add exactly one record for the new logical ID to `expected-results.toml`.
   Use an explicit `not-yet-classified` record with a reason when evidence does
   not support a complete classification. Do not infer metadata from the
   fixture path or payload.
6. Run `make check-conformance-manifest`,
   `make check-conformance-expected-results`, and the
   `conformance-test-support` tests.
7. For an AG4 parser case, require a modern canonical AE V2/V3 execution model,
   active AE disposition, exact AG/AE ID and input reconciliation, and run
   `make check-conformance-parser`. Do not construct `ValidatedFixtureSpec`,
   parse AE sidecars in AG, or copy Borrowser output as an oracle.
   A `dom-tree` package declares only the canonical tree expectation; parser
   errors, document mode, diagnostics, unsupported features, and final
   invariants are reportable actual evidence rather than extra DOM-equivalence
   predicates.
8. For an AG5 CSS case, use outer V2 plus one strict nested
   `borrowser-css-fixture-v1` descriptor. Declare every selector list,
   stylesheet, document/fragment input, expected snapshot, and package file.
   Property/value declares the phase-specific singular
   `[input].stylesheet` carrier and no cascade metadata. Combined cascade
   profiles declare a bounded ordered `stylesheets` list;
   every item has `path`, `origin`, `order`, and `source` (user-agent input also
   declares its namespace). A fragment request additionally declares its
   context namespace and local name; it is representable but not executable.
   The nested profile determines the required and forbidden fields and whether
   outer `test_path` names the selector-list or profile-specific stylesheet
   input. Selector matching intentionally uses its selector list as `test_path`;
   its required HTML request is explicit support context and is never inferred.
   Target addresses start at the document root and index every ordinary child,
   including text, comments, processing instructions, and doctypes. Every step
   must select an element with the declared namespace/local name, so the path is
   an assertion about the traversed parser-created tree, never a selector or an
   engine ID. Outer/nested reconciliation belongs to
   the runner adapter; `css-test-support` validates only the nested CSS package
   and does not depend on generic AG crates. Run `make check-conformance-css`.
9. For an AG7 static reference case, use outer V3 plus one strict
   `borrowser-paired-rendering-fixture-v1` descriptor. Put the descriptor, test
   HTML, reference HTML, and every stylesheet beneath the execution-package
   root. Declare test and reference through their dedicated outer fields;
   support paths contain only remaining nested files. Each side declares full
   origin/order/source/namespace stylesheet coordinates. Select only Layout
   profiles for `layout-geometry` or only Paint profiles for
   `paint-operations`. Run `make check-conformance-rendering`.
10. For the one AG8 derived case, use outer V4 and link it to the authoritative
    external-source lineage. WPT interpretation and source-record accounting
    remain in `wpt-test-support`; subsystem adaptation remains in the relevant
    subsystem test-support crate. Run `make check-conformance-wpt` before the
    ordinary manifest, expected-results, and rendering checks. A derived AG7
    semantic result is never a WPT raster pass.

`fixture.toml` is the source of truth. `manifest.toml` is a checked-in,
deterministically generated review artifact and must not be edited by hand.
The check operation is read-only and reports a missing or stale manifest.

`expected-results.toml` is also a source of truth and is edited by hand. Its
check operation is read-only: it validates strict typed metadata against the
complete discovered inventory and prints an ephemeral deterministic summary.
There is intentionally no update mode and no checked-in generated summary.
When changing an expectation, capability, harness limitation, stability
declaration, environment requirement, or lane exclusion, include the required
reason and relevant contract/tracking reference. Engine availability must be
supported by authoritative production contracts or deterministic tests; a
fixture's presence and expected-pass declaration are not evidence.
Before marking a case classified, verify that its payload supplies every
context needed to identify the logical observation without invention. Harness
readiness must separately account for delegation, whether an authoritative
expected observation has been authored, whether an existing expectation can be
represented truthfully, the subsystem-owned observation, and comparison
infrastructure; a missing adapter is not automatically the only limitation.

Fixture payloads are byte-preserving inputs. Do not assume that `.html`,
`.css`, `.txt`, or any other extension permits line-ending normalization or
UTF-8 decoding. Git attributes intentionally mark all bundle content as binary
first and opt only `fixture.toml` back into LF-normalized infrastructure text.

Inventory presence means only that a descriptor was discovered and validated.
It does not mean the fixture is runnable, passes, conforms to a standard, is a
WPT reftest, or demonstrates browser compatibility.

AG4 executes `html-tokenizer`, `html-tree-construction`, and `dom-tree`
packages. AG5 executes the seven focused CSS profiles documented in
[`docs/conformance/ag5-css-conformance-runners.md`](../../docs/conformance/ag5-css-conformance-runners.md).
Generic AG metadata decides eligibility; the HTML adapter
delegates one canonical evaluation to `html-test-support`, which may execute
all baseline/parity parser strategies. AG6/AG7 Layout and Paint cases use the
separate rendering lane. Browser/runtime observation, broad WPT/html5lib
import, WPT reftest execution, pixels, cross-engine execution, JavaScript, DOM
APIs, events, and dynamic behavior remain outside the current lanes. AG7
static relations compare subsystem-owned structural bytes and add neither WPT
source adaptation nor raster support. AG8 adds controlled source adaptation but
still adds no raster support. AG5 CSS execution
ends at `ComputedDocumentStyle`; fragment requests are representable but remain
capability-unavailable until HTML provides contextual fragment parsing.
The subsystem-neutral runner has no adapter feature enabled by default;
repository commands explicitly enable `html-parser`, `css`, or `rendering`.
