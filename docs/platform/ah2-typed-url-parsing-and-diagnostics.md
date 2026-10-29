# AH2: Typed URL Parsing, Resolution, And Diagnostics

Last updated: 2026-10-29
Status: AH3 explicit-base URL resolution acceptance satisfied; document-level base integration remains deferred

The sections preceding the [2026-10-28 integration](#ah3-maintained-fork-integration-2026-10-28)
retain the original AH2 decision and AH3 blocked checkpoint as historical evidence.
They describe published `url 2.5.7`, not the dependency selected after this integration.
The [2026-10-29 operation-debugging contract](#ah3-structural-operation-debugging-2026-10-29)
supersedes the historical userinfo-only Debug policy with content redaction.
The [final native acceptance audit](#ah3-final-native-acceptance-2026-10-29) records
AH3 closeout evidence; earlier pending/blocked statements are historical checkpoints.

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

Historical AH2 Debug policy (superseded by the current operation-debugging
contract below): `Debug` is deterministic structured component output with a fixed field order.
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
## AH3 explicit-base qualification: blocked acceptance (2026-10-27)

AH2 merged through PR #1440. AH3 starts at master
`0109ab0c489891c0ff3d57f1d4cd446c656dcaa9` on
`feat/AH3-base-url-resolution`, with a clean working tree and unchanged
production `BrowserUrl`, diagnostics, dependency graph, and consumers. This is
an incomplete AH3 evidence checkpoint, not an implementation closeout.

### Unchanged base and diagnostic boundary

A parsed, immutable `BrowserUrl` supplies explicit base context to `resolve`;
a new base type or document state is not needed. A malformed base string fails
at `BrowserUrl::parse`, before resolution is possible. Without base context,
`parse` accepts absolute inputs and reports `RelativeWithoutBase` for relative
inputs. A successfully parsed opaque-path base supports fragment references,
but not empty, query-only, or path-relative references. Parsing/resolution is
separate from resource-operation support: parseable unsupported-operation
schemes do not become syntax errors. No capability registry or document base
selection is added by AH3.

Outcomes retain operation-local recoverable diagnostics in callback order,
including repetitions; failures retain typed fatal categories and the supplied
base. Base-parsing diagnostics belong to base construction and are not
replayed by resolution. These are existing AH2 contracts; expanded native AH3
regressions remain unfinished because the discovery gate below blocked work.

### Population and selection, established before execution

The unchanged pinned corpus contains 1,011 entries: 896 objects and 115 comment
strings. Of the objects, 336 have explicit non-null string bases: 274 concrete
successful URL expectations and 62 expected failures. All 336 were evaluated.
The remaining 560 objects have null bases and are outside this discovery pass;
AH2's existing selected absolute qualification remains separate.

Seventeen of those null-base objects also carry `relativeTo` annotations
(16 `non-opaque-path-base`, one `any-base`). All seventeen explicitly expect
failure without a base; none supplies a concrete resolved expectation for a
substituted base. They were not expanded into invented base/result pairs.
The pinned [upstream constructor harness](https://github.com/web-platform-tests/wpt/blob/c48d58747e1f211527fb695fd60548a997fae617/url/url-constructor.any.js)
uses `base` and `failure` and does not interpret `relativeTo`. Its blob identity
is `b4b639b813e9752db24c38e94dcf57c084ee52fd`. AH3 does not claim coverage of
annotation-driven expansions, URL mutation, origins, or JavaScript URL APIs.

The [32-case selection](../../crates/addressing/tests/fixtures/wpt/resolution-selection.json)
was recorded before any resolution results were evaluated. It covers ordinary
paths, dot segments, root/authority replacement, query/fragment/empty inputs,
same-scheme shorthand, fatal input, non-special hierarchy, opaque bases,
file URLs, and known file host/drive/separator risk classes. These semantic
criteria did not depend on whether the implementation passed. The manifest
retains exact source indices, input/base identities, individual rationales,
corpus/license hashes, and the existing WPT and Standard revisions. The original
absolute selection, source corpus, license, and expectations remain unchanged.

### Discovery results and attribution

The new production-API discovery test checks all ten existing URL component
projection fields against WPT, or its explicit expected fatal result. It also
checks repeated typed outcomes and base immutability. It does not reparse
production serialization to create an observation. Separate calls directly to
locked `url 2.5.7` verify wrapper/backend component and outcome parity solely
for attribution; they are not the normative oracle. All bases parsed, and all
336 production outcomes matched the direct dependency outcomes.

| Scope | Strict WPT matches | Discrepancies | Interpretation |
| --- | --- | --- | --- |
| Focused 32 | 29 | 3 | Differences at 645, 667, 993 are existing file-defect classes. |
| All 336 explicit-base cases | 303 | 33 | 241 successful URLs and all 62 expected failures matched. |
| Existing file-defect classes | 0 of these 22 discrepant cases | 22 | Additional manifestations of AH-URL-001, not strict passes. |
| Newly discovered non-file discrepancies | 0 of these 11 discrepant cases | 11 | Material acceptance blockers, not accepted limitations. |

The 22 file discrepancies below belong to the previously documented file
host/drive and empty-path-segment defects. They remain dependency-owned and
could be considered only under AH2's explicit limitation policy, with no
conformance claim. Four cases already have a changed parsed base (613, 646,
660: host lost; 703: empty segment lost). The remaining 18 first diverge during
reference resolution. This distinction prevents blaming resolution alone for
base-parsing information loss. No new file correction is implemented.

All cells below use JSON string spelling so backslashes and newlines remain
unambiguous. `InvalidInput(EmptyHost)` denotes the typed fatal actual result.
Component-level differences and parsed-base serializations are also printed by
the reproducible test; the tables retain each discrepancy's complete URL result.

| WPT index | Reference | Base input | Expected URL | Actual URL |
| --- | --- | --- | --- | --- |
| 613 | `"/"` | `"file://h/C:/a/b"` | `"file://h/C:/"` | `"file:///C:/"` |
| 635 | `"/////mouse"` | `"file:///elephant"` | `"file://///mouse"` | `"file:///mouse"` |
| 637 | `"\\/localhost//pig"` | `"file://lion/"` | `"file:////pig"` | `"file:///pig"` |
| 638 | `"//localhost//pig"` | `"file://lion/"` | `"file:////pig"` | `"file:///pig"` |
| 639 | `"/..//localhost//pig"` | `"file://lion/"` | `"file://lion//localhost//pig"` | `"file://lion/localhost//pig"` |
| 645 | `"C\|"` | `"file://host/dir/file"` | `"file://host/C:"` | `"file:///C:"` |
| 646 | `"C\|"` | `"file://host/D:/dir1/dir2/file"` | `"file://host/C:"` | `"file:///C:"` |
| 647 | `"C\|#"` | `"file://host/dir/file"` | `"file://host/C:#"` | `"file:///C:#"` |
| 648 | `"C\|?"` | `"file://host/dir/file"` | `"file://host/C:?"` | `"file:///C:?"` |
| 649 | `"C\|/"` | `"file://host/dir/file"` | `"file://host/C:/"` | `"file:///C:/"` |
| 650 | `"C\|\n/"` | `"file://host/dir/file"` | `"file://host/C:/"` | `"file:///C:/"` |
| 651 | `"C\|\\"` | `"file://host/dir/file"` | `"file://host/C:/"` | `"file:///C:/"` |
| 658 | `"/c:/foo/bar"` | `"file://host/path"` | `"file://host/c:/foo/bar"` | `"file:///c:/foo/bar"` |
| 660 | `".."` | `"file://x/C:/"` | `"file://x/C:/"` | `"file:///C:/"` |
| 666 | `"C\|/"` | `"file://host/"` | `"file://host/C:/"` | `"file:///C:/"` |
| 667 | `"/C:/"` | `"file://host/"` | `"file://host/C:/"` | `"file:///C:/"` |
| 668 | `"file:C:/"` | `"file://host/"` | `"file://host/C:/"` | `"file:///C:/"` |
| 669 | `"file:/C:/"` | `"file://host/"` | `"file://host/C:/"` | `"file:///C:/"` |
| 699 | `"file:////one/two"` | `"file:///"` | `"file:////one/two"` | `"file:///one/two"` |
| 702 | `"////one/two"` | `"file:///"` | `"file:////one/two"` | `"file:///one/two"` |
| 703 | `"file:///.//"` | `"file:////"` | `"file:////"` | `"file:///"` |
| 993 | `"/\\//\\/a/../"` | `"file:///"` | `"file://////"` | `"file:///"` |

The eleven new cases below are outside the documented file/encoding/IDNA
limitations. Seven valid HTTP references fail unexpectedly; three non-special
backslash references resolve to incorrect paths or authorities; one non-special
parent traversal incorrectly treats a drive-shaped segment as a file root.
Their base URLs parse unchanged. Direct dependency parity attributes all eleven
to `url 2.5.7`, rather than the Borrowser adapter. They pre-exist AH3: the
production code is byte-identical to merged AH2. "New" means newly discovered
in this qualification, not introduced by this change.

| WPT index | Reference | Base input | Expected URL | Actual URL or failure |
| --- | --- | --- | --- | --- |
| 659 | `".."` | `"abc://x/y/z/C:/"` | `"abc://x/y/z/"` | `"abc://x/y/z/C:/"` |
| 984 | `"///test"` | `"http://example.org/"` | `"http://test/"` | `"InvalidInput(EmptyHost)"` |
| 985 | `"///\\//\\//test"` | `"http://example.org/"` | `"http://test/"` | `"InvalidInput(EmptyHost)"` |
| 986 | `"///example.org/path"` | `"http://example.org/"` | `"http://example.org/path"` | `"InvalidInput(EmptyHost)"` |
| 987 | `"///example.org/../path"` | `"http://example.org/"` | `"http://example.org/path"` | `"InvalidInput(EmptyHost)"` |
| 988 | `"///example.org/../../"` | `"http://example.org/"` | `"http://example.org/"` | `"InvalidInput(EmptyHost)"` |
| 989 | `"///example.org/../path/../../"` | `"http://example.org/"` | `"http://example.org/"` | `"InvalidInput(EmptyHost)"` |
| 990 | `"///example.org/../path/../../path"` | `"http://example.org/"` | `"http://example.org/path"` | `"InvalidInput(EmptyHost)"` |
| 1004 | `"\\a"` | `"foo://foo/a"` | `"foo://foo/\\a"` | `"foo://foo/a"` |
| 1005 | `"\\/a"` | `"foo://foo/a"` | `"foo://foo/\\/a"` | `"foo://a"` |
| 1006 | `"\\\\a"` | `"foo://foo/a"` | `"foo://foo/\\\\a"` | `"foo://a"` |

These new differences block honest AH3 acceptance under the approved scope.
In particular the non-special backslash cases can change the resolved authority,
and valid HTTP slash forms are rejected. They have not been added to an accepted
inventory or disguised as passes. No local parser patch, fork, second resolver,
or normalization workaround is proposed. A qualified upstream correction or an
explicit project decision on these additional limitations is required before
AH3 can be closed. Such a decision is not inferred from AH2's narrower inventory.

### Reproduction and incomplete validation

Run the strict discovery test (it deliberately exits 101 with all 33 differences
visible until the acceptance blockers are resolved):

```sh
cargo test -p addressing --test qualification pinned_wpt_relative_discovery --locked -- --nocapture
```

The JSON report includes each source index, input, base input, actual parsed
base, and per-component independently expected and actual values. All report
inputs are fixed public synthetic WPT fixture data. This is conformance evidence,
not an arbitrary-user-input logging facility or a safe composite debug API.
No credential-bearing runtime input is collected. The existing production
`Debug` contract redacts only userinfo; path/query/fragment contents are still
inspectable and can contain secrets.

Before discovery, `cargo test -p addressing --test browser_url --locked` passed
all nine existing tests. The discovery test fails on the material new dependency
behavior, so AH3 implementation stops at this checkpoint as requested. Expanded
native regressions and successful/failed/sensitive composite debug snapshots
have not been implemented; the debugging exit criterion is still open. Document
URL/fallback/active base lifecycle, HTML base selection, HTML/CSS consumer
migration, origins, resource identity, fetching, caching, and navigation remain
deferred. No production Rust types, APIs, algorithms, state, logging, or consumer
behavior changed.

| Validation at this checkpoint | Result |
| --- | --- |
| `cargo test -p addressing --test browser_url --locked` | PASS: nine existing tests, before adding discovery. |
| `cargo test -p addressing --test qualification pinned_wpt_relative_discovery --locked -- --nocapture` | FAIL (101): all 336 evaluated, 33 discrepancies retained; repeated run confirms the same counts. |
| `cargo test -p addressing --locked` | FAIL (101): all 19 existing tests pass; only the new strict discovery test fails. No ignored tests; doc tests are not reached after the integration-test failure. |
| `make fmt-check` | PASS. |
| `cargo clippy -p addressing --all-targets --locked -- -D warnings` | PASS. |
| `git diff --check` | PASS. |
| Runtime smoke | Not needed: no production/runtime behavior changed. |
| `make ci` | Not run: implementation stopped at the explicit material-defect gate; no readiness or clean-CI claim. Required before a later completed implementation is declared ready for review. |

Raw logs are retained locally in `target/ah3-validation/`; their essential
results, expectations, and reproduction commands are recorded here so the
evidence does not depend on untracked build artifacts. No commits, pushes,
PRs, or issue changes were made.

## AH3 maintained-fork integration (2026-10-28)

### Exact source and qualified graph

The approved immutable parsing/explicit-base dependency is maintained
[`joris97jansen/rust-url`](https://github.com/joris97jansen/rust-url) revision
`7dc4ecbf4d29817e723e04ae94b14f1326764f61` (`url 2.5.8`). This is the production
revision, not CI control commit `32630e7b55f8ad75a3f92e96a48552dac755f35c`.
[Hosted qualification run 40706692979](https://github.com/joris97jansen/rust-url/actions/runs/40706692979)
qualified Linux, Windows, macOS, native feature lanes, sanitizers, consumer-graph
fuzz regression replay, audit and pinned WPT outcomes before this integration.

The original Borrowser consumer lock SHA-256 is
`3802ddab6a9d38ed2228ff165244fd30ba91523522f3e7e11e276bb9225c3ab3`.
It exactly matches the consumer lock recorded in fork qualification. The fork's
qualification workspace lock is a different file with SHA-256
`a770f264be9656bcf64db971b60c2a09ee0a542600ead12b66f52f878db154d6`;
its extra development dependencies are not consumer dependencies.

Cargo graph trials establish the minimal overrides:

- Root `[patch.crates-io]`: `url` and `percent-encoding`, each pinned to the full
  production revision above. Patching `url` automatically selects its sibling
  Git/path `idna 1.1.0`, `form_urlencoded 1.2.2` and `percent-encoding 2.3.2`.
  No explicit IDNA or form patch is necessary. The percent-encoding patch unifies
  `winit` and `arboard` with that existing fork instance; a URL-only trial produced
  duplicate registry/Git instances at the same version.
- Independent fuzz workspace: only the revision-pinned `url` patch. Its actual
  graph already contains one instance of each fork sibling, so extra sibling
  overrides would add no necessary selection.
- The existing workspace `url = "2"` requirement remains sufficient. The full
  revision and committed locks select `2.5.8`; an `=2.5.8` requirement is not
  necessary for this integration. Locked source selection is checked explicitly.

The root lock changes only URL's version (`2.5.7` to `2.5.8`) and the four fork
package source identities. Every external version and registry checksum remains
unchanged. The fuzz lock's previous ICU 2.2 graph is aligned to the already
qualified consumer closure, not upgraded to the modern qualification graph.
Changes outside that closure are excluded. Both resolved URL closures contain
exactly the qualified 35 coordinates (URL plus 34 runtime dependencies), with
one instance of each fork package and no unused patches. External registry
checksums match qualification.

| Dependency | Preserved qualified version |
| --- | --- |
| `idna_adapter` | `1.2.1` |
| `icu_collections`, `icu_locale_core`, `icu_normalizer`, `icu_normalizer_data`, `icu_provider` | `2.0.0` |
| `icu_properties`, `icu_properties_data` | `2.0.1` |
| `potential_utf`, `tinystr`, `zerotrie`, `zerovec` | `0.1.3`, `0.8.1`, `0.2.2`, `0.11.4` |
| `serde`, `serde_core`, `serde_derive` | `1.0.225` |

Direct/transitive URL consumers in the main graph are addressing, Browser, CSS,
GFX, ureq and webbrowser. The fuzz workspace reaches URL through CSS. All resolve
the same approved revision. This is a single dependency source override, not a
second URL resolver or a consumer normalization repair. Graph changes require
explicit review and requalification rather than an unbounded dependency update.

Reproduction after integration:

```sh
cargo metadata --locked --format-version 1
cargo tree --locked -i url
cargo tree --locked -i percent-encoding
cargo metadata --manifest-path fuzz/Cargo.toml --locked --format-version 1
cargo tree --manifest-path fuzz/Cargo.toml --locked -i url
cargo test -p addressing --locked -- --nocapture
cargo test -p browser -p net --lib --locked
```

### rust-url synchronization monitor

The dedicated [rust-url workflow](../../.github/workflows/rust-url-sync.yml) runs
Mondays at 06:23 UTC and supports manual execution. It uses read-only permissions
and temporary complete Git histories; it never changes dependencies or remote
branches. Its network checks run separately from PR validation. Deterministic
tests join the existing `make check-url-dependencies` discovery.

Missing upstream ancestry in the maintained `borrowser` branch, or a fork `main`
that differs from upstream `main`, produces a review warning and exit 1. A newer
maintained revision beyond the consumer pin is informational (exit 0): adoption
requires explicit qualification. Conflicting or malformed main/fuzz pins are
configuration errors. Unavailable references, failed/timed-out fetches, unrelated
histories and incomplete comparisons are errors (exit 2), which take precedence
over warnings. An ahead or divergent consumer history also requires review.
The Actions summary records hashes, counts, bounded commit lists and comparison
links; branch-based comparison links show live branch state.

Git ancestry proves historical reachability, not semantic equivalence: cherry-picks
and squashes can warn despite equivalent changes, while later reverts preserve
ancestry. This monitor establishes neither dependency qualification nor URL
Standard conformance. The existing CI check verifies the previously qualified
Cargo graph.

GitHub currently identifies `master` as Borrowser's default branch. Merging this
workflow there enables the next weekly schedule, without triggering a run on the
merge itself. After merging, manually run and inspect it:

```sh
gh workflow run rust-url-sync.yml --repo joris97jansen/borrowser --ref master
```

Verify the run uses the merged `master` SHA and Python 3.12.15, inspect all
applicable comparisons and their summaries, and confirm the conclusion matches
the findings.
A legitimate synchronization warning is a failed monitor, not a deployment
failure. Then confirm the next scheduled run occurs. Manual runs inspect their
selected ref; scheduled runs inspect the default branch.

Failure notifications depend on GitHub Actions email/web notification settings;
scheduled recipients follow the workflow creator, later cron editor or person
who re-enabled it. Availability notices do not trigger failure-only notifications.
GitHub may delay or drop scheduled runs and disables public-repository schedules
after 60 days without activity. The monitor cannot detect its own missing runs.
See [GitHub's event documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#schedule)
and [notification documentation](https://docs.github.com/en/actions/concepts/workflows-and-actions/notifications-for-workflow-runs).

### Typed diagnostic integration

`SyntaxViolation::DomainToAscii` maps explicitly to `UrlDiagnostic::DomainToAscii`.
It reports strict domain-to-ASCII validation and can accompany a successful
non-strict parse: `https://xn--/` is accepted with this event. A fatal invalid
domain remains `InvalidInput(InvalidDomain)`; a later invalid port remains
`InvalidInput(InvalidPort)`. Prior events retain order and repetitions, and
resolution failures retain the supplied base with credential-redacted Debug.
Unknown future callbacks still map to `Unclassified`.

Focused regressions cover acceptance, a valid IDNA control, callback ordering,
fatal domain/port failures, deterministic repetition, immutable base context,
and absence of replayed base-construction events. The existing
`FileHostWithDriveLetter` callback remains observed for `file://host/C:/` even
though its host now survives parsing; integration preserves that callback
contract rather than inventing diagnostic silence.

### Corrected outcomes and preserved evidence

Before removing the nine historical parsing deviation records, the unchanged
54-case qualification selected zero deviations under the pinned dependency.
The old inventory deliberately failed on that resolved difference. The active
inventory is now empty; original observed values remain in this document's
historical tables and Git history. No WPT expectations or selections changed.

Production `BrowserUrl` qualification now requires all **560/560 null-base** and
**336/336 explicit-base** WPT outcomes, comparing all ten component fields or
the independently specified failure outcome. The original 54 absolute and 32
relative selection identities remain unchanged. Repeatability and base immutability
are checked. The 17 null-base `relativeTo` annotations still receive no invented
base/result expansions. These are parsing/resolution claims, not setter claims.

Native regression checks now require caret percent encoding, file host/path
identity and the former rust-url #1101 fixed point. Image-source and Tab tests
require the corrected request/resource keys. A loopback transport regression
requires raw and encoded caret references to use `/a%5Eb` on the wire. Parsing
of `https://xn--/` is checked at the typed boundary; it is no longer used as a
supposed local parser rejection in a network test. Production Browser and net
implementations remain unchanged.

The WPT corpus, licenses, selection manifests and source expectations remain
byte-identical. Diagnostic callbacks are checked against explicit state contracts;
the WPT component fixtures are not represented as a complete diagnostic oracle.

### Limitations and AH3 completion boundary

The original ten Unicode 17 IDNA corpus failures belong to the modern ICU 2.4 /
Unicode 18 reference graph. They remain documented failures, not passes. The
qualified consumer graph uses ICU 2.0 / Unicode 16. Neither conformance fixture
nor generic IDNA behavior is changed in Borrowser.

The fork still records nine setter findings: eight implementation limitations
and one missing href-dispatch case in its existing driver (the public href API
conforms). Immutable `BrowserUrl` exposes none of those setters. This integration
does not qualify mutation APIs or erase those findings.

Dependency integration alone does not close [AH3 #1113](https://github.com/joris97jansen/borrowser/issues/1113)
or Milestone AH. The full local CI pipeline now passes. Remaining AH3 acceptance
includes a final audit of the focused native missing/invalid-base, invalid-reference
and unsupported-scheme cases, and credential-safe operation-level Debug output
covering input, base, resolved URL and diagnostics with deterministic snapshots.
Existing value/outcome/failure Debug tests do not establish that complete operation
record. AH3 explicitly permits closeout before document-level base integration.
Active/fallback base state and lifecycle, HTML base elements, origin/security policy,
fetch/resource identity and further consumer migrations remain separate later work.
No new base type, parser abstraction, scheme registry, or consumer-side workaround
is introduced by this integration.

### Integration validation

The focused typed diagnostic tests and the complete addressing package pass:
one library test, 11 typed URL tests and 11 qualification tests. The WPT totals
are 560/560 null-base, 336/336 explicit-base and 54/54 original absolute-selection
matches, with zero parsing deviations. Browser's 248 tests and net's 14 tests pass,
including deterministic image/resource keys and loopback caret normalization.
Both complete Cargo graphs match the qualified URL dependency closure/checksums,
and all existing fuzz bins pass a locked compile check.

`make ci` passes locally with Rust 1.92.0 on native macOS. This includes its fixed
conformance suite, formatting, all three warnings-denied Clippy lanes, both
workspace test lanes, parser/golden/WPT/runtime/fuzz smoke checks, debug and release
builds, bench compilation and generated-entity validation. Release/bench builds
still emit non-fatal warnings in unchanged HTML/CSS files; these are not suppressed.
No known failure was converted into a pass, no expectation was skipped, and no
live site is used as conformance evidence.


## AH3 structural operation debugging (2026-10-29)

### Production capability and operation association

`addressing::format_resolution_debug(reference, base, &result)` formats an already
completed parse or explicit-base resolution. It borrows `&str`,
`Option<&BrowserUrl>` and `&Result<UrlParseOutcome, UrlFailure>` and returns an
owned, sanitized `String`. It does not parse or resolve again, retain the input
in an outcome/failure, clone URLs or diagnostic collections, mutate values,
perform I/O, or introduce automatic logging. There is no report/context type,
extra resolver, logging dependency, raw-output mode, or document base state.

```rust
use addressing::{BrowserUrl, format_resolution_debug};

let base = BrowserUrl::parse("https://example.test/dir/page").unwrap().url;
let reference = "../image.png";
let result = base.resolve(reference);
let diagnostic = format_resolution_debug(reference, Some(&base), &result);
```

The caller must provide input, base and result from the same completed operation.
The formatter cannot prove their provenance. A failure already retains a typed
base: the formatter compares that base with the supplied base using full URL
`Eq`, including credentials, private components and `None` versus `Some`.
`failure_base_matches: Some(true)` means those bases compare equal; `Some(false)`
exposes inconsistent context without changing or rejecting the result. Identical
redacted URL output does not establish equality. For success this check is
unavailable and the field is `None`. No check authenticates the reference or a
successful result, and public result fields are not an integrity boundary.

The deterministic field order is `input`, `base`, `resolved`, `diagnostics`,
`failure`, `failure_base_matches`. On success, `resolved` is `Some` and `failure`
is `None`; on failure the reverse applies. Diagnostics remain in callback order,
including repetitions and events preceding fatal errors. Base-construction
events are not replayed. Base context is explicit, not document-selection
provenance. A malformed base string remains a separate base-construction parse
failure; no invalid `BrowserUrl` is manufactured for diagnostic output.

### Current privacy policy

Production-safe structural diagnostics deliberately withhold exact URL content:

| Field | Representation |
| --- | --- |
| Reference | Empty string stays empty; every nonempty input becomes `<redacted>`, including malformed references. |
| Scheme | Only literal labels `http`, `https`, `ftp`, `file`, `ws`, `wss`, `about`, `data`, `blob`, `javascript` are displayed; all other scheme values become `<redacted>`. This is a formatting vocabulary, not a scheme capability registry. |
| Username/password | Normalized absence/empty/nonempty distinctions remain; nonempty contents become `<redacted>`. |
| Host | Only absent/present and `domain`, `opaque`, `ipv4`, `ipv6` kind are displayed as `host_kind`. No hostname or address is emitted. |
| Port | Absent/present remains; every explicit nondefault value becomes `<redacted>`. Default ports already normalize to absence. |
| Path and opaque payload | Empty versus nonempty remains; nonempty content becomes `<redacted>`, including embedded URLs and file paths. |
| Query/fragment | `None`, `Some("")` and `Some("<redacted>")` distinguish absence, empty and nonempty. Parameter names, values and fragment contents are all withheld. |
| Authority/opaque path | Existing normalized boolean properties remain visible. |
| Diagnostics/failure | Existing typed enum categories; no raw input or backend error-display strings. |

No sensitive lengths, hashes, fingerprints or raw substrings are added. The
formatter explains status, component presence and validation/failure categories;
it does **not** reveal the exact authored reference or URL transformation. Many
distinct URLs intentionally have the same diagnostic projection. Structural
metadata, standard scheme labels, diagnostic categories and base-consistency
results remain observable, so this is not a promise of universal information
hiding or safe logging of arbitrary application data.

`BrowserUrl::Debug` now applies the same content policy, in ordinary and pretty
formatting. Derived `UrlParseOutcome::Debug`, `UrlFailure::Debug`, and nested
`Result`/`Option` output inherit it. `UrlFailure::Display` emits only the typed
failure category. Neither `BrowserUrl` nor `UrlParseOutcome` implements Display.
The historical host/path/query/fragment/port Debug contents are deliberately
removed; parsing, semantic equality/hash and serialization are unchanged.

Complete accessors (`as_str`, `scheme`, `host`, credentials, path, query, fragment,
port) still expose their actual values. `UrlHost`'s derived Debug and the returned
host/IP values' Debug/Display can expose host contents. Formatting those explicit
accessors, raw references, complete strings or backend URL values is outside this
privacy guarantee. The formatter does not call those raw formatting paths as an
intermediate shortcut. Its output is an internal regression contract, not URL
serialization, resource identity, a public API schema or a transport format.

### Independent evidence and remaining acceptance

The production-API tests in `crates/addressing/tests/resolution_debug.rs` use
reviewed literal success/failure/empty/missing-base/opaque snapshots, including
nested pretty Debug. Synthetic secret substitutions protect host, IP, credential,
path, query, fragment, port and custom-scheme redaction without collapsing actual
URL equality. Tests cover malformed/encoded inputs, deterministic repetition,
ordered repeated diagnostics, base-event isolation, failure-base mismatches,
owned input lifetime and immutable URL/hash/result semantics.

Exact URL transformations remain independently verifiable through typed
accessors in controlled synthetic tests. Those assertions are conformance
evidence; they do not require a production raw-output mode. Normative WPT corpus,
expectations and qualification remain unchanged: 560 null-base and 336
explicit-base cases must continue to match. Diagnostic completeness remains
limited to observed backend events; no new validation taxonomy is invented.

The operation-debugging implementation passed validation. At this checkpoint,
AH3 still required its final native acceptance audit of missing/invalid bases,
invalid references and parseable unsupported schemes. That audit is completed
in the final acceptance section below.
No document-level base lifecycle, HTML base integration, origins, capability
policy, fetching, navigation or consumer migration is introduced here.


## AH3 final native acceptance (2026-10-29)

The final audit read [AH3 #1113](https://github.com/joris97jansen/borrowser/issues/1113),
the AH1/AH2 ownership contracts, production APIs and existing native/WPT evidence.
The issue explicitly permits closeout before document-level base integration.
The parsed, immutable `BrowserUrl` supplied to `resolve` is the explicit typed
base. A new base/context/report abstraction or document state is not required.
A parseable opaque URL is a valid URL even though some relative forms cannot
resolve against it. Unsupported resource operations remain separate from syntax.

The read-only audit found no production defect and exactly two missing permanent
assertions: empty references against an opaque base, and typed fatal failure for
an authority-relative reference with an invalid port. Temporary audit probes
confirmed existing behavior before the assertions were added. Both now extend
existing functions in `crates/addressing/tests/browser_url.rs`; no new test module,
production API, parser behavior, fixture, dependency or browser consumer changed.

### Completed native acceptance matrix

Every row below is satisfied. Test links identify concrete existing assertions;
WPT references are zero-based indices in the unchanged pinned corpus.

| Requirement | Native tests / qualification | Asserted behavior and invariant |
| --- | --- | --- |
| Absolute URLs without a base | [`normalized_components_are_explicit`](../../crates/addressing/tests/browser_url.rs#L10); [`pinned_wpt_absolute_discovery`](../../crates/addressing/tests/qualification.rs#L198) | Typed components and normalized serialization; all 560 null-base outcomes match. |
| Relative references without a base | [`fatal_errors_retain_prior_events_without_raw_input`](../../crates/addressing/tests/browser_url.rs#L226); [`missing_base_snapshot_and_failure_consistency_use_actual_url_equality`](../../crates/addressing/tests/resolution_debug.rs#L112) | Empty/path-relative inputs fail with RelativeWithoutBase and no supplied base. |
| Invalid base construction | [`fatal_errors_retain_prior_events_without_raw_input`](../../crates/addressing/tests/browser_url.rs#L226) | The shared BrowserUrl::parse constructor rejects invalid host/domain/port/IP inputs with typed failures before a base can be supplied. Private URL storage prevents bypassing construction; no nominal base wrapper is needed. |
| Explicit typed base and common relative forms | [`explicit_base_resolution_preserves_the_base_and_reference_semantics`](../../crates/addressing/tests/browser_url.rs#L91); [`domain_to_ascii_validation_is_recoverable_and_ordered`](../../crates/addressing/tests/browser_url.rs#L338); [`pinned_wpt_relative_discovery`](../../crates/addressing/tests/qualification.rs#L52) | Simple/parent/root/scheme-relative paths, dot segments, query/fragment/empty forms and absolute overrides produce independently expected URLs. WPT 263 covers a literal dot reference. |
| Opaque bases: rejected references | [`explicit_base_resolution_preserves_the_base_and_reference_semantics`](../../crates/addressing/tests/browser_url.rs#L91); [`pinned_wpt_relative_discovery`](../../crates/addressing/tests/qualification.rs#L52) | Path and now empty references fail with RelativeAgainstOpaqueBase. The empty case explicitly asserts retained base, no callbacks, immutability and identical repeated errors. WPT 404–405 independently reject query-only references. |
| Opaque bases: permitted references | [`explicit_base_resolution_preserves_the_base_and_reference_semantics`](../../crates/addressing/tests/browser_url.rs#L91); [`opaque_resolution_snapshot_hides_payload_and_preserves_failure_distinction`](../../crates/addressing/tests/resolution_debug.rs#L151) | Fragments and absolute references succeed; data URL payloads remain opaque and redacted in diagnostics. |
| Malformed authority-relative reference | [`fatal_errors_retain_prior_events_without_raw_input`](../../crates/addressing/tests/browser_url.rs#L226) | HTTPS base plus //user:password@authority.test:70000/path fails with InvalidPort and exactly EmbeddedCredentials. The exact typed base is retained and unchanged; repeated errors, including diagnostic order, compare equal. |
| Recoverable events preceding fatal errors | [`fatal_errors_retain_prior_events_without_raw_input`](../../crates/addressing/tests/browser_url.rs#L226); [`domain_to_ascii_validation_does_not_replace_fatal_errors`](../../crates/addressing/tests/browser_url.rs#L385); [`failed_resolution_snapshot_retains_prior_events_and_fatal_category`](../../crates/addressing/tests/resolution_debug.rs#L63) | Prior callbacks remain separate from typed fatal categories, including domain-to-ASCII validation before a later port failure. |
| Ordering, repetitions and deterministic results | [`recoverable_events_are_ordered_repeated_and_separate_from_values`](../../crates/addressing/tests/browser_url.rs#L185); [`recoverable_order_and_repetitions_are_not_replayed_from_base_construction`](../../crates/addressing/tests/resolution_debug.rs#L323) | Literal callback vectors retain order and duplicates; repeated parse/resolve/format results compare equal. |
| Base immutability and event isolation | [`domain_to_ascii_validation_is_recoverable_and_ordered`](../../crates/addressing/tests/browser_url.rs#L338); [`pinned_wpt_relative_discovery`](../../crates/addressing/tests/qualification.rs#L52); [`successful_resolution_snapshot_preserves_exact_semantics_separately`](../../crates/addressing/tests/resolution_debug.rs#L18) | Base construction events are not replayed. All 336 explicit-base cases check repeatability and base immutability; both added fatal cases retain and preserve their supplied bases. |
| Query-only, fragment-only and empty references | [`explicit_base_resolution_preserves_the_base_and_reference_semantics`](../../crates/addressing/tests/browser_url.rs#L91); [`absent_and_empty_components_and_credentials_follow_normalization`](../../crates/addressing/tests/browser_url.rs#L59); [`empty_reference_snapshot_distinguishes_empty_from_withheld_input`](../../crates/addressing/tests/resolution_debug.rs#L94) | Exact query/fragment results, fragment removal by an empty reference, and absent versus present-empty components remain explicit; opaque-empty behavior is separately rejected. |
| Parseable unsupported schemes and resource-operation boundary | [`parsing_does_not_imply_operation_support_or_origin_computation`](../../crates/addressing/tests/browser_url.rs#L295); [`pinned_wpt_relative_discovery`](../../crates/addressing/tests/qualification.rs#L52); [`unsupported_and_rejected_urls_fail_before_a_resource_response`](../../crates/net/src/tests/fetch.rs#L25) | Valid file/FTP/data/javascript/blob/custom URL syntax remains parseable. WPT 45, 93–95, 104–106 and 112 cover nonstandard/unsupported references with explicit bases. Existing resource-operation failures remain Transport with no response; transport is not a URL-validity oracle. |
| Typed results, serialization, equality and hashing | [`url_equality_and_hashing_use_complete_serialization_not_origin_or_fetch_identity`](../../crates/addressing/tests/browser_url.rs#L159); [`pinned_wpt_absolute_semantics_and_published_dependency_deviations`](../../crates/addressing/tests/qualification.rs#L405); [`successful_resolution_snapshot_preserves_exact_semantics_separately`](../../crates/addressing/tests/resolution_debug.rs#L18) | Results use BrowserUrl; exact synthetic strings/components, fixed points, full-serialization identity and before/after hashes remain checked. Diagnostic projections are not identity keys. |
| Credential-safe operation debugging | [`successful_resolution_snapshot_preserves_exact_semantics_separately`](../../crates/addressing/tests/resolution_debug.rs#L18); [`failed_resolution_snapshot_retains_prior_events_and_fatal_category`](../../crates/addressing/tests/resolution_debug.rs#L63); [`component_presence_and_nested_pretty_debug_have_explicit_snapshots`](../../crates/addressing/tests/resolution_debug.rs#L177); [`secret_substitution_preserves_structural_debug_without_collapsing_url_identity`](../../crates/addressing/tests/resolution_debug.rs#L241) | Nine production-API tests protect literal success/failure snapshots, nested ordinary/pretty redaction, sensitive substitutions, malformed references, provenance limitations and no retained input. |

### Final validation and acceptance scope

The updated native BrowserUrl suite passes all 11 tests. The complete addressing
suite passes all 32 tests: one library, 11 typed URL, 11 qualification and nine
operation-debugging tests. Test counts do not increase merely because two
assertion groups were added. Independent pinned WPT remains **560/560 null-base**
and **336/336 explicit-base**, zero discrepancies; the original 54 absolute and
32 focused relative selections also pass. No normative expectation was changed.

Commands run for the final assertion increment:

```sh
cargo test -p addressing --test browser_url --locked --offline -- --nocapture
cargo test -p addressing --locked --offline -- --nocapture
make check-url-dependencies
cargo fmt --all -- --check
cargo clippy -p addressing --all-targets --locked --offline -- -D warnings
git diff --check
make ci
```

All pass. The full CI run includes the existing warnings-denied Clippy lanes,
workspace/feature tests, deterministic runtime/golden/fuzz checks, release builds,
bench compilation and generated entities. Existing nonfatal release/bench warnings
remain unsuppressed. The acceptance documentation is recorded after successful
validation; the tested Rust content remains unchanged. Final documentation and
whitespace checks pass.

Both Cargo workspaces still select qualified rust-url revision
`7dc4ecbf4d29817e723e04ae94b14f1326764f61` and its approved 35-package closure.
Production parsing, resolution, serialization, equality, hashing, callbacks and
structural Debug are unchanged by the final assertions. Historical Unicode/IDNA
and setter findings above remain documented; this is scoped immutable URL
acceptance, not complete library-wide WHATWG or mutation conformance.

**AH3's exit criteria are satisfied and it is ready for closeout review.** This
records local implementation/validation status, not a claim that a GitHub issue
has been closed, changes merged or a PR published. No further implementation
phase or abstraction is required. Milestone AH is not complete: document URL and
fallback/active-base lifecycle, HTML base integration, origins, further consumer
migrations, resource/cache identity, fetching/navigation and security policy
remain outside this issue's accepted scope.
