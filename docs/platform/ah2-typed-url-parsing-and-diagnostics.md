# AH2: Typed URL Parsing, Resolution, And Diagnostics

Last updated: 2026-10-26
Status: implemented and ready for code review with explicit dependency limitations;
full CI reproduces native macOS process-ownership failures present on clean master

## Scope and revised architectural decision

AH2 provides a Borrowser-owned URL boundary under the
[AH1 architecture contract](ah1-url-origin-resource-addressing-contract.md).
It wraps the published Rust `url` crate; it does not implement a parser or
import any experimental upstream correction. WHATWG remains the semantic authority.

The 2026-10-26 architectural decision explicitly replaces the earlier requirement
that every selected backend conformance case pass before AH2 production work.
Published `url 2.5.7` is accepted with recorded external limitations. Upstream
repairs, a private fork, and complete library-wide WHATWG qualification are not
prerequisites. This changes the dependency acceptance gate, not the independent
standards expectations. No failing input has been removed from the corpus or
rewritten before parsing. The old gate failed and is not retroactively a PASS.

Original AH2 required absolute parsing and was closeable before relative
resolution. The resumed task explicitly adds pure resolution against a supplied
`BrowserUrl`, including `../images/logo.png`, and allows the existing image-source
helper to use it. This bounded extension does not implement document base
selection, base-element processing, origins, resource identity, or later AH work.

## Baseline, ownership, and dependency direction

The implementation resumes `feat/AH2-typed-url-parsing` at AH1 merge
`7b2189c0b5fe8570fa7113de6d0e925e81941635`, containing AH1 commit
`406974ea694ad64d240f0692f8ec405a9468fc18`. The pre-existing staged changes were
AH2 qualification files and their documentation; no unrelated work was carried in.

`addressing` is a dedicated low-level crate depending only on `url` in production.
Browser now depends on addressing. The existing `platform` crate depends on
Browser's downstream consumers (GFX, net, HTML and runtime services); placing the
parser there would reverse dependency direction. `core_types` remains a small
transport/identity crate rather than acquiring parser semantics. The owner is
justified by the URL model and the current image-source consumer.

Cargo.lock continues to select the same published parser and IDNA graph used by
existing direct consumers. No source dependency, patch, second normalization
backend, or dependency version upgrade is introduced.

## Public contract

[`BrowserUrl`](../../crates/addressing/src/lib.rs) privately owns exactly one
`url::Url`. It is immutable and cloneable, with no setters, backend escape hatch,
duplicated component storage, factory, context object, or parser trait.

| API | Contract |
| --- | --- |
| `BrowserUrl::parse(&str) -> Result<UrlParseOutcome, UrlFailure>` | Absolute parsing without an implicit base, address-bar guessing, or producer-specific trimming. |
| `url.resolve(&str) -> Result<UrlParseOutcome, UrlFailure>` | Parse an absolute or relative reference against this explicit base, without modifying it. No document/base lifecycle is inferred. |
| `as_str() -> &str` | Complete backend-normalized serialization, including credentials, query, and fragment. |
| `scheme() -> &str` | Lowercase scheme without colon; no loader-capability assertion. |
| `username() -> &str` | Normalized percent-encoded username; empty does not identify authored presence. |
| `password() -> Option<&str>` | Normalized percent-encoded password; authored empty passwords normalize to `None`. |
| `host() -> Option<UrlHost<'_>>` | Borrowed domain/opaque host or typed standard-library IPv4/IPv6 address. No backend host type escapes. |
| `port() -> Option<u16>` | Explicit nondefault port; scheme defaults normalize away. |
| `path() -> &str` | Serialized hierarchical or opaque path; percent encoding is preserved. |
| `query()`, `fragment()` | `Option<&str>` without delimiter; absent and present-empty remain distinct. |
| `has_authority()`, `has_opaque_path()` | Preserve empty-authority versus hostless hierarchy and opaque-path distinctions. |

`UrlHost::Domain` and `Opaque` distinguish special-scheme domain processing from
case-sensitive non-special hosts. IPv6 is an address value, not bracketed text.
An empty host is `None`; `has_authority()` preserves the difference between
`custom:///x` and `custom:/x`. The special-scheme classification supplies host
semantics only, not a speculative fetch-capability registry.

Credential examples: `https://u:@h.test/` serializes as `https://u@h.test/`,
with username `u` and password `None`; `https://@h.test/` and `https://:@h.test/`
normalize to `https://h.test/`. `https://:p@h.test/` retains an empty username and
`Some("p")`. Encoded credentials remain encoded; no authored-presence flags exist.

### Identity, serialization, and debug

`Eq` and `Hash` use the complete normalized URL serialization. Credentials,
query order/repetitions, empty query/fragment markers, fragment contents, and
percent-escape spelling participate. Diagnostics do not. Normalized default
ports and dot segments can make differently authored inputs equal.

URL equality is neither origin equality nor a fetch/cache key. Two paths on the
same authority can be unequal URLs; fragments are retained. No origin API is
exposed, and the backend's origin accessor is never called. Later AH origin
work must preserve tuple/opaque-origin lifecycle and the unresolved file-origin
policy from AH1 rather than deriving security identity from this equality.

Repeated `as_str()` calls are stable because the URL is immutable. Ordinary
HTTP/HTTPS and the tested relative references also satisfy parse-serialize-parse
stability. Universal standards-normalization and round-trip stability are not
claimed: the published dependency has the retained file counterexample below.
No ad hoc normalization repairs or blanket rejection of file URLs are applied.

`Debug` is deterministic structured component output with a fixed field order.
Username/password contents become `<redacted>` while their normalized presence
remains inspectable. Both normal and pretty Debug, including nested outcomes,
are safe for URL userinfo. Debug is an internal regression contract, not a public
wire format. `as_str()` and explicit credential accessors intentionally remain
complete and must not be used as credential-safe logging. Path, query and fragment
are inspectable and are not a general secret-scrubbing facility.

### Outcomes, failures, and diagnostics

`UrlParseOutcome { url, diagnostics }` separates successful parsing from the
ordered `Vec<UrlDiagnostic>`. Events stay in callback invocation order, including
repetitions; they are never sorted or deduplicated. They describe that operation's
input, not the permanent URL. Resolution does not replay the base's diagnostics.
No source positions, completeness claims, or missing validation events are invented.

`UrlFailure { kind, diagnostics, base }` retains prior events even when parsing
fails. `base` is `None` for absolute parsing and a clone of the explicit base for
resolution failure, meeting AH1's base-context requirement without global state
or a new context type. Its Debug redacts credentials. No raw input or backend
display string is stored. `UrlFailureKind` distinguishes
`InvalidInput(UrlParseError)` from backend `CapacityExceeded` and `BackendFailure`.
The producer still owns document/base-selection provenance; `RelativeWithoutBase`
and `RelativeAgainstOpaqueBase` distinguish base failures. Backend size overflow is
not mislabeled invalid syntax. Setter-only and unknown future backend errors
map conservatively to `BackendFailure`; unknown callback variants map to
`Unclassified`. Allocation failure/panic is not promised to be recoverable.

Borrowser fatal categories cover empty host, invalid domain, invalid domain
character, invalid port, invalid IPv4/IPv6, and the two base errors. Recoverable
categories cover boundary-control/space removal, tab/newline removal, authority
and file-slash recovery, embedded credentials, unencoded `@`, backslash,
file-host/drive interaction, non-URL code points, NULL fragment and invalid
percent encoding. These are observed backend events, not a complete WHATWG
validation taxonomy. In particular the backend's NULL wording is not adopted as
a claim that the character was dropped: the normalized fragment retains `%00`.

`http://0x7f.1/` and `custom://%/` parse without events despite standard validation
conditions. `https://exam%70le.test/` is a valid escaped-domain control, not a
missing-error claim. Silent callbacks do not establish conformance.

Parseable `file:`, `ftp:`, `data:`, `javascript:`, `about:`, `blob:`, and unknown
schemes remain parseable. Unsupported resource operations are semantically
separate and are not fabricated as parse errors or implemented in AH2.

## Consumer integration and deferred migrations

Only [`browser::resources::resolve_image_source`](../../crates/browser/src/resources.rs)
migrates: it strips HTML ASCII whitespace, rejects empty input, first tries
absolute parsing, then parses its supplied base and resolves the reference.
Its `Option<String>` boundary, failure fallback, credentials/query/fragment,
resource strings, lookup keys, and resource loading behavior are preserved.
The consumer intentionally discards diagnostics, as before; addressing makes
them available for later Browser reporting without adding logging side effects.

Browser document-style stylesheet resolution, tab discovery, navigation
normalization/comparison, labels, and GFX URL helpers remain direct consumers of
the same locked parser. They require their own context/compatibility work; this
change does not redesign them or claim all engine callers are migrated.
No CSS, HTML, Layout, Paint, GFX, networking, cache, or resource-manager identity
ownership moves. There is no global base or URL state.

Document/fallback/active base lifecycle, stylesheet provenance, origins,
same-origin comparison, resource identities, scheme-operation policy, network
fetching changes, and new AG observation families remain later AH responsibilities.

## Consumer impact assessment

The resumed acceptance review keeps the approved API and published dependency.
It assesses active consumers, not a new backend qualification gate.

The image pipeline has two independent resolution calls using the same locked
parser. `BrowserReplacedElementInfo::resolve_image_source` calls the AH2 helper
when building Layout's image presentation/lookup data. It does not initiate a
fetch. `Tab::discover_images` still parses the page base with `url::Url`, joins
HTML image references, and supplies the normalized string to
`ResourceManager::request_image`. That manager uses the string verbatim as its
lookup/deduplication key and sends it unchanged in `CoreCommand::FetchStream`.
`runtime_net` forwards it to `net::fetch_stream`; response/chunk bookkeeping
continues to use the requested string. AH2 did not migrate these other callers.

| Authored input | `BrowserUrl::as_str()` and AH2 image helper | Pre-AH2 helper | Downstream handling on macOS |
| --- | --- | --- | --- |
| `file://example.net/C:/` | `file:///C:/` | Same | `file://` dispatch; `File::open` receives `/C:/`. No remote-host operation is attempted. |
| `file:///w\|/m` | `file:///w\|/m` | Same | `File::open` receives `/w\|/m`; the parser did not normalize the drive separator. |
| `file://host//w\|/m` | `file://host/w\|/m` | Same | `File::open` receives the relative platform path `host/w\|/m`. No second URL parse drops the remaining host here. |
| `file:////w\|/m` | `file:///w\|/m` | Same | `File::open` receives `/w\|/m`; the serialized empty path segment is already lost. |
| `file://spider///` | `file://spider/` | Same | `File::open` receives relative `spider/`. |
| `file:////foo` | `file:///foo` | Same | `File::open` receives `/foo`. |
| `http://example.test/a^b` | Raw caret retained | Same | `ureq` reparses with `url 2.5.7`; the request writer sends `/a^b`. |
| `http://example.test/a%5Eb` | Encoded caret retained | Same | Distinct key and request path `/a%5Eb`. |
| `non-special:opaque  ?hi` | Two spaces retained | Same | Discovery enqueues it, but the HTTP client's host check rejects it before connecting. |
| `https://xn--/` | Typed `InvalidDomain`; helper `None` | `None` | Image discovery rejects it before queuing a fetch. |

The `File::open` arguments above are traced from the actual source, not evidence
that these specific files exist or load successfully. There is no standards-aware
file URL conversion in `net::file::fetch_file_url`: it strips the prefix and, on
Windows only, one leading slash. The current loader supports local-file I/O but
has no remote file-authority implementation. A parser correction alone would not
make that loader standards-compliant. Redundant platform separators may refer
to the same file; changed serialized keys do not by themselves prove different
filesystem contents. Native Windows loading was not executed.

For other schemes `net::fetch_stream_with_policy` calls `ureq 2.12.1`.
`Request::parse_url` reparses and requires a hostname; `connect_socket` accepts
HTTP/HTTPS (its internal `test` branch does not supply a production transport).
`send_prelude`/`PreludeBuilder::write_request_line` write the parsed path without
an extra caret-encoding pass. The loopback test observes the bytes actually sent
and independently asserts raw versus encoded paths and unchanged response URL
metadata. It contacts no external site and establishes no exploitability or
particular server-side interpretation. Hostful unknown schemes and rejected
IDNA inputs also return transport errors without response events.

Comparison baseline is fetched `origin/master`,
`7b2189c0b5fe8570fa7113de6d0e925e81941635`. The resource-helper test includes its
small pre-AH2 body verbatim in behavior (qualified `url::Url` and renamed function)
and asserts both it and AH2 against literal observations. The original WHATWG
expectations stay separate and unchanged. The parser dependency graph is identical
for registry versions/checksums. No normalization or loading difference was found
for these inputs; AH2's intended changes are typed ownership and diagnostics.

Focused evidence:

- `addressing::tests/browser_url`: file host-loss, repeated slashes, caret,
  opaque-path and IDNA observations alongside independent standards results.
- `browser::resources::tests::known_url_deviations_match_pre_ah2_image_resolution`:
  absolute references, relative caret path, encoded control and fatal IDNA parity.
- `browser::tab::tests::network::known_url_deviations_reach_image_requests_and_resource_lookup_keys`:
  actual HTML publication, discovery, bus fetch strings, and resource lookups.
- `net::tests::fetch::image_requests_preserve_raw_caret_and_encoded_caret_as_distinct_wire_paths`:
  actual loopback HTTP traffic.
- `net::tests::fetch::unsupported_and_rejected_urls_fail_before_a_resource_response`:
  no-response rejection of opaque/unsupported/IDNA cases.

This replaces the earlier narrow impact assessment in AH-URL-001. Both the file
and caret deviations reach active code; the caret affects HTTP addressing and
wire serialization. They predate AH2. No parser rewrite, second backend, loader
change, scheme-policy expansion, or workaround is proposed in this issue.

## Validation and interpretation

The original strict audit was rerun before reclassifying the gate: exit 101,
6 test functions passed and 3 failed. The 54 unchanged selected WPT cases still
contain 45 standards matches and 9 deviations. Native normalization still has
25 matches and the caret discrepancy; the issue-1101 file round trip remains
unstable. These are dependency failures, not 54 conformance passes.

The current suite compares every selected case against the unchanged corpus and
asserts the exact discrepancy inventory in
[`published-url-deviations.json`](../../crates/addressing/tests/fixtures/published-url-deviations.json).
New or resolved differences fail the test and require review. No skip/ignore or
fixture regeneration hides failures. The caret and file round-trip observations
are independently explicit. Canonical dependency tracking is
[AH-URL-001](../engine-feature-gap-tracker.md#ah-url-001--upstream-whatwg-url-parsing-deviations).
Other previously discovered opaque-path, caret, and ASCII-IDNA differences remain
visible below; accepting the dependency does not relabel them correct.

The public API tests assert literal expected results independently of backend
calls: components, credentials, absent/empty values, equality/hash identity,
relative and absolute references, failures, repeated event order, redacted debug,
and complete serialization. Browser tests independently cover the existing
resource-provider behavior. Synthetic backend limit mapping is tested without
allocating a multi-gigabyte input. No full relative-resolution or cross-platform
qualification claim follows from these representative cases.

### Initial implementation validation on 2026-10-26

Environment: macOS arm64, `rustc 1.92.0 (ded5c06cf 2025-12-08)`, Cargo 1.92.0.

| Level | Executed command/check | Actual result |
| --- | --- | --- |
| 1: targeted | `cargo test -p addressing --locked --offline` | PASS: 19 tests, no ignored cases; includes exact known-deviation assertions, not a complete conformance pass. |
| 1: consumer | `cargo test -p browser --locked --offline resources::tests` | PASS: 5 tests. |
| 2: formatting/lint | `make fmt-check`; `lint`, `lint-html5`, `lint-html5-hardening` through `make ci` | PASS, including the final base-provenance error representation. |
| 2: default workspace | `cargo test --workspace --exclude borrowser-conformance --lib --bins --tests --examples --locked` | PASS: 2,314 tests across 61 binaries, zero ignored. Browser: 246 unit and 5 integration tests. The excluded package's actual failure is recorded below. |
| 2: HTML5 workspace | Same workspace command with `--features html5` | PASS: 2,314 tests, zero ignored. |
| 3: deterministic smoke | `make conformance-ci`; `make test-html5-smoke-real-pages` | PASS. No GUI/live-site inspection was used as correctness evidence. |
| 4: full CI | `make ci`, first sandboxed and then with normal host permissions | FAIL, exit 2 both times: conformance native process-ownership tests, 177 passed / 9 failed / 13 existing ignored. Not an AH2 test failure and not reported as a CI pass. |
| 4: remaining CI targets | `make -k` with the remaining targets listed below | PASS, exit 0. |
| Review | Complete diff, `git diff --check`, dependency graph/checksums, original fixture bytes | PASS. Registry dependencies did not change; original WPT corpus, selection and license are unchanged. |

After the full-CI stop, these remaining Make targets were executed together:

```sh
make -k test-html5-parser-conformance test-html5-external-fixtures \
  test-html5-parser-fatal-failures test-html5-dom-golden test-html5-patch-golden \
  test-html5-smoke-real-pages test-html5-rawtext-script-regressions \
  test-html5-tokenizer-fuzz-smoke test-html5-tokenizer-script-data-fuzz-smoke \
  test-html5-tokenizer-rawtext-fuzz-smoke test-html5-tokenizer-rcdata-fuzz-smoke \
  test-html5-tree-builder-token-fuzz-smoke test-html5-pipeline-regressions \
  test-html5-pipeline-fuzz-smoke test-wpt-tree-builder build build-html5 \
  build-release build-release-html5 compile-html5-benches compile-css-benches \
  html-entities-check
```

The nine failures are unchanged `chromium::tests` cases:
`native_capture_failure_diagnostics`, `native_cleanup_diagnostic_failures`,
`native_lifecycle_regressions`, `native_macos_bsd_ownership_states`,
`native_process_and_pipe_contract`, `native_resource_completion_and_shutdown`,
`native_shutdown_uses_capture_deadline`, `native_startup_exit_event_classification`,
and `native_temporary_artifact_containment`. Reported failures include
`task_name_for_pid: Mach 5` and `live process executable unavailable during
ownership scan`, followed by cleanup timeouts. Retrying outside the sandbox did
not resolve them. Their source was not changed or skipped in the CI command.
Separate workspace runs exclude the failed package explicitly, not silently.

Release/benchmark builds also emitted existing HTML/CSS unused-code warnings;
the repository fuzz-smoke scripts emitted sanitizer-symbol/coverage warnings.
Those commands exited 0; this is smoke evidence, not sanitizer qualification.
No new test skips or warning suppressions were introduced.

AH2's typed representation, component APIs, deterministic outcomes, serialization,
equality, tests, documentation and narrow integration satisfy the issue under the
revised published-dependency decision. Relative resolution is the explicit bounded
addition requested at resumption. Architecture review found no parser workarounds,
duplicated persistent parser state, consumer API leaks, or subsystem ownership
changes. Code review can proceed; clean full-CI sign-off remains outstanding.

### Acceptance follow-up validation on 2026-10-26

The [clean-master attribution record](../conformance/ag2-local-chromium-capture.md#ah2-clean-master-attribution-check-2026-10-26)
contains the exact nine-test command, environment, detached worktree/build
provenance, per-test signatures and outcomes. Both master and AH2 failed all nine
selected tests with matching signatures, under identical host permissions.
This replaces the previous inference based only on unchanged test source.
The originating macOS/process inspection condition remains unknown.

Only tests and documentation changed in this follow-up. The accepted addressing
implementation, dependency graph, URL algorithms, image helper production body,
and resource loader behavior were preserved. The complete final diff was checked
for ownership, borrowing, error handling, test independence and scope; no parser
workaround, new skip, normalization special case or API redesign was introduced.

| Check | Command | Result |
| --- | --- | --- |
| Addressing | `cargo test -p addressing --locked --offline` | PASS: 19 tests, with expanded independent dependency/consumer observations. |
| Browser resource helper | `cargo test -p browser --locked --offline resources::tests` | PASS: 6 tests. |
| Browser consumer trace | `cargo test -p browser --locked --offline known_url_deviations` | PASS: 2 tests, including HTML discovery and actual resource keys; rerun after the fixture borrowing fix. |
| HTTP transport | `cargo test -p net --locked --offline tests::fetch` | PASS: 3 tests, including distinct raw/encoded caret wire paths and unsupported/rejected URL errors. Loopback only. |
| Master/AH2 native comparison | Exact command in linked conformance record | Both FAIL: exit 101, 9/9 selected tests fail, matching signatures; master stays clean. |
| Default workspace | `cargo test --workspace --exclude borrowser-conformance --lib --bins --tests --examples --locked` | PASS: 2,318 tests in 61 binaries; zero ignored. Browser has 248 unit plus 5 integration tests; net has 14 tests. |
| HTML5 workspace | Same workspace command with `--features html5` | PASS: 2,318 tests; zero ignored. |
| Formatting/Clippy | `make ci` formatting and all three lint lanes | Final run PASS. The first attempt found a needless borrow in the new HTML test fixture; corrected without suppression and revalidated. |
| Full CI | `make ci` outside the sandbox, after that correction | FAIL: exit 2, exactly the nine baseline native tests; conformance test binary reports 177 passed / 9 failed / 13 existing ignored. |
| Diff/provenance | `git diff --check`, registry and normal dependency-feature graph comparison, baseline helper source comparison | PASS; the normal conformance dependency/feature graph differs only by the accepted local `addressing` package. |

The full-CI conformance suite itself passes (seven fixture cases); the failure is
in the subsequent native helper unit tests. The separately excluded conformance
package is not reported as passing. Targets after this CI stop retain their
initial implementation results above; they were not silently counted as rerun.
Raw logs, exact commands and dependency graphs are retained locally in
`target/ah2-acceptance/`. The earlier validation remains separately recorded in
`target/ah2-validation/`.

No AH2-specific normalization/loading regression or unresolved implementation
failure was found. The accepted published-backend decision still applies despite
confirmed active-path limitations. AH2's functional acceptance evidence is ready
for final review. An unconditional clean-CI merge recommendation remains pending
a successful required-CI run or an explicit project decision accepting the
reproduced baseline failure; this does not require a parser workaround or a new
Borrowser issue. Conformance/platform owns the native failure, in the existing
record linked above.

## Reproduction and dependency provenance

Run from the repository root:

```sh
cargo test -p addressing --test qualification --locked -- --nocapture --test-threads=1
```

Observed on macOS with `rustc 1.92.0 (ded5c06cf 2025-12-08)` and Cargo 1.92.0.
The tests use no network, filesystem URL conversion, renderer, or JavaScript.

| Dependency | Evaluated selection |
| --- | --- |
| `url` | crates.io 2.5.7; workspace declaration remains `url = "2"` |
| `url` package checksum | `08bc136a29a3d1758e07a9cca267be308aeebf5cfd5a10f3f67ab2097683ef5b` |
| `url` source revision from package metadata | `2387efa12d3d294d488bb77ab580a21b88868fd2` |
| IDNA implementation | `idna 1.1.0`, `idna_adapter 1.2.1` |
| Unicode processing | `icu_normalizer 2.0.0`, `icu_properties 2.0.1` |
| Fixture reader / digest | `serde 1.0.225`, `serde_json 1.0.145`, `sha2 0.10.9` |

The complete graph is reproducible with `cargo tree -p addressing --locked`.
The recorded backend observations apply to this locked graph; a newer release or upstream
revision is not qualified by these results.

## Independent expectations and selection

Semantic authority is the [WHATWG URL Standard](https://url.spec.whatwg.org/),
inspected at its 2026-10-19 revision
[`845bafca22fb0c5a42c4bf384a9bdea525a2f10d`](https://github.com/whatwg/url/tree/845bafca22fb0c5a42c4bf384a9bdea525a2f10d).
Native expectations refer to absolute parsing, host parsing, percent-encoding,
serialization, and URL equivalence; no RFC parser is used as a substitute.

The unchanged [WPT corpus](../../crates/addressing/tests/fixtures/wpt/urltestdata.json)
comes from [web-platform-tests revision
`c48d58747e1f211527fb695fd60548a997fae617`](https://github.com/web-platform-tests/wpt/blob/c48d58747e1f211527fb695fd60548a997fae617/url/resources/urltestdata.json).
Its [BSD license](../../crates/addressing/tests/fixtures/wpt/LICENSE.md) is
retained. The [selection manifest](../../crates/addressing/tests/fixtures/wpt/selection.json)
records 54 explicit indices, input identities, selection rationales, and these
SHA-256 digests:

- Corpus: `81e85fd3c199c08ef9c34cf651b3580eeedd080316493bfaf277a6b5ff8cf652`.
- License: `5fac07febb0e2a97fb0d7b0def149ec08b642e1ba4b9c345283ab1cbd2af6570`.

The test verifies those digests, unique selections, source input identity, and
`base: null` before using the upstream expectations. It compares `href`,
`protocol`, `username`, `password`, `host`, `hostname`, `port`, `pathname`,
`search`, and `hash`, or the independently specified fatal result. WPT origin
fields and cases requiring a base are not evaluated. Shipping the entire source
file preserves provenance; it does not imply that the entire corpus ran.

Native cases use literal expected results written independently of the candidate.
Callback enum expectations additionally characterize the inspected backend
contract; they are not a claim that WPT defines callbacks. The separate upstream round-trip reproduction retains the fixed-point invariant
as a known dependency failure, not a claim that the first serialization is correct.

## Retained standards discrepancies

These are mismatches with unchanged WPT expectations, except the explicitly
identified native case. Spaces inside code spans are significant.

| Input | Independently expected normalized result | Actual result | Violated requirement |
| --- | --- | --- | --- |
| `file:///w\|/m` | `file:///w:/m` | `file:///w\|/m` | Windows drive normalization; WPT 138 |
| `non-special:opaque  ?hi` | `non-special:opaque %20?hi` | `non-special:opaque  ?hi` | Opaque-path trailing-space encoding; WPT 292 |
| `file://spider///` | `file://spider///` | `file://spider/` | Preserve empty path segments; WPT 630 |
| `file://localhost//a//../..//` | `file://///` | `file:///` | Correct path normalization after localhost handling; WPT 634 |
| `file://example.net/C:/` | `file://example.net/C:/` | `file:///C:/` | Preserve domain host; WPT 662 |
| `file://1.2.3.4/C:/` | `file://1.2.3.4/C:/` | `file:///C:/` | Preserve IPv4 host; WPT 663 |
| `file://[1::8]/C:/` | `file://[1::8]/C:/` | `file:///C:/` | Preserve IPv6 host; WPT 664 |
| `file:////foo` | `file:////foo` | `file:///foo` | Preserve empty host and path shape; WPT 697 |
| `https://xn--/` | `https://xn--/` | Fatal `IdnaError` | Current all-ASCII domain acceptance; WPT 921 |
| `http://example.test/a^b` | `http://example.test/a%5Eb` | `http://example.test/a^b` | Path percent-encode set; native case |

The literal input from [rust-url issue 1101](https://github.com/servo/rust-url/issues/1101)
is also retained:

```text
input:             file://.cRe!+aacRddddddddddddddtpe=//t:/a|et/!..
first serialization: file://.cre!+aacrddddddddddddddtpe=/t:/a|et/!..
after reparsing:    file:///t:/a|et/!..
```

The independent requirement is that reparsing a successful serialization preserves
that URL, including its host, components, and equality. Here all three checks
fail. The exact first output differs from the upstream reporter's dependency
environment; the table records this workspace's actual result, without importing
the reporter's output as an oracle. The first serialization is itself not a
claim of correct canonical normalization.
