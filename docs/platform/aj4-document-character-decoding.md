# AJ4: HTML Document Character Decoding

Issue: [#1135](https://github.com/joris97jansen/borrowser/issues/1135).
Milestone AJ. Base: `a7eac925af1bd60d0584fa37eade1666fe2f8be5`.

## Ownership and production path

Browser selects the supported document encoding from AI8's immutable aggregate
Content-Type. Fetch still owns header extraction, HTTP decompression and byte
integrity. HTML owns the existing byte decoder, preprocessing and parsing.
CSS, Layout, Paint and retained rendering gain no decoding responsibility.

The existing path remains NetworkResponseInfo → Browser preparation/activation →
ParseHtmlStart/Chunk/Done → runtime_parse → HtmlParser::push_bytes →
ByteStreamDecoder/tools::utf8 → decoded Input/HTML preprocessing → parser output.
Existing HtmlParseFinished events carry decoder evidence to Browser. There is no
second decoder, full-body buffer, dependency or decoder-selection framework.
BodyProgress continues counting delivered decompressed response bytes, independently
of scalar output and replacement expansion.

## Supported selection policy

After AJ3's provenance, disposition, status and declared-HTML gates:

| Selected charset fact | Decision |
| --- | --- |
| Absent | UTF-8; MissingCharsetDefault. |
| Empty after ASCII whitespace trimming | UTF-8; EmptyCharsetDefault. |
| Supported UTF-8 label | UTF-8; ResponseCharset. |
| Other nonempty label | UnsupportedDocumentCharset response rejection. |

Supported labels are `unicode-1-1-utf-8`, `unicode11utf8`, `unicode20utf8`,
`utf-8`, `utf8`, and `x-unicode20utf8`. Trim only U+0009, U+000A, U+000C,
U+000D and U+0020, then compare ASCII case-insensitively. No Unicode whitespace,
punctuation rewriting, IANA lookup or speculative alias registry is used.
These follow the [Encoding Standard label table](https://encoding.spec.whatwg.org/#names-and-labels).
Unknown and known unsupported labels share the same current capability outcome.

Missing charset does not admit missing Content-Type. AJ3 still rejects missing,
empty, unusable and non-HTML MIME. Browser consumes AI8's selected aggregate
charset, including its duplicate-parameter and repeated-field selection rules;
it does not choose another first/last header. Original metadata remains retained.
204/205 do not select an HTML encoding or start a parser.

All selection failures precede parser-start enqueue and activation. A rejected
pending request preserves the actual active DOM, history, addressing, inputs,
stylesheet work and retained render state. Existing cancellation and first-failure
rules apply. Accepted default/declared UTF-8 uses the already-fixed UTF-8 parser
byte entry; no additional ParseHtmlStart configuration is needed.

## Recovery and authoritative diagnostics

The existing constrained tools::utf8 state machine emits InvalidSequence or
IncompleteSequenceAtEof replacement events. Invalid sequences become U+FFFD;
a valid incomplete prefix is carried between chunks and becomes a replacement
only at actual EOF. A literal U+FFFD produces no decoder issue.

Utf8DecodeCounts holds two saturating u64 event counts: invalid_sequences and
incomplete_sequences_at_eof. The ordinary counted sink and optional observed
sink classify the same authoritative replacement callbacks. Category counts
replace the stored backend aggregate; their saturating sum supplies decode_errors.
There is no separately updated total, Browser counter or diagnostic event log.
Accounting remains mandatory with observation disabled or capacity exhausted.
Canonical AE diagnostics retain their existing detailed identities and payloads.

The stable HtmlParseCounters.decode_errors field and semantics are unchanged.
Low-level html::html5::Counters deliberately changes decode_errors from a field
to a derived accessor. In-repository backend consumers are updated explicitly;
low-level tooling must migrate to decode_errors(). Category storage is crate-private;
backend struct-literal callers must construct via Default instead. No such
external struct-literal consumers exist in this repository.

HtmlDecodeReport contains those counts and eof_processed. The latter records
return from the actual decoder EOF phase, before further parser work can fail.
Empty carry, zero errors, body completion and parser failure stage cannot derive
that fact. Read-only report access works after parser failure and drains nothing.
It introduces no new standalone parser mutation or post-EOF lifecycle policy.

## Terminal evidence and interruption

Existing success, parser execution/finalization failure, and InputClosed events
carry available reports. Initialization/handle failure carries None. InputClosed
reports the consumed prefix without flushing the decoder or manufacturing EOF.
Successful parser completion must include finalized decoding evidence.

Cancellation stays silent. Repeated starts silently retire the old instance and
its counts, including when replacement initialization fails. At most one terminal
send is attempted per runtime session instance; retirement and closed sinks can
produce no delivered terminal. A runtime key can identify successive instances,
so no global exactly-one-event guarantee is claimed. Remaining sessions emit
InputClosed in the existing sorted order when the command channel closes.

Browser retains reports only through existing current-generation terminal
admission. Invalid success cannot install evidence. An accepted parser failure
retains its report with the original cause. A failed Browser document cannot be
enriched by later events. Existing late-completion invariant checks remain.
Transport failure followed by silent cancellation normally leaves decoder
counts unavailable; this is not zero errors or successful decoding. No report
acknowledgement, cancellation event or asynchronous enrichment protocol is added.

## BOM/meta limitations and standards divergence

BOM sniffing, switching and removal are deferred together. EF BB BF is decoded
as U+FEFF and passed to HTML preprocessing/tokenization. It is not stripped.
That scalar is non-whitespace in Initial mode and can select quirks before a
following doctype, which is then too late to select no-quirks mode. Deterministic
whole/split-byte tests cover retained input and this document-mode consequence.
Truncated BOM prefixes use ordinary UTF-8 EOF recovery; UTF-16 signatures receive
no special treatment. An unsupported declared charset is rejected before body
inspection, even if a later BOM could override it in a complete browser.

HTML meta charset declarations are parsed as HTML but do not select an encoding,
trigger a prescan or restart parsing. No legacy decoder, encoding confidence,
locale/parent/history heuristic, user override or stylesheet encoding is added.
The [HTML input-byte-stream rules](https://html.spec.whatwg.org/multipage/parsing.html#the-input-byte-stream)
and [Encoding BOM processing](https://encoding.spec.whatwg.org/#decode) have wider
requirements. This contract states Borrowser's restricted subset, not full WHATWG
HTML encoding conformance. Future work belongs at the existing response-admission
and parser-input boundaries; no placeholder framework is installed.

## Observation and validation boundary

Loading diagnostics show selected MIME classification, actual encoding decision,
fixed BOM/meta policy, EOF-qualified decode outcome and both counts. Unsupported
charset rejection is explicit. Missing evidence remains absent. Output uses fixed
order, no raw charset/body/URL values, the existing sheet-row limit and 16 KiB
snapshot envelope. Formatting does not run selection, decoding or scheduling.

Validation results are recorded separately for the final reviewed candidate;
implementation capability does not claim green CI, full assembly or AJ completion.

## Candidate validation (2026-11-23)

The reviewed uncommitted candidate starts at the base above. Its changed crate
source/fixture fingerprint is
`f523db402631bcf030a8466992f994fb718d7ccaf7e60b2fea3a3d835b39efd8`:
SHA-256 over sorted changed crate paths, NUL, file bytes, NUL; Markdown is excluded.

| Level / check | Fresh result |
| --- | --- |
| Targeted `aj4_` HTML/runtime/Browser regressions | 4 / 4 / 10 passed, including canonical observation and failure injection where applicable. |
| `cargo test -p browser --locked` | 431 unit and 5 integration tests passed. |
| Browser with `conformance-observation` | 442 unit and 5 integration tests passed. |
| HTML / runtime fatal-failure feature lane | 973 / 35 unit tests passed. |
| Dependency qualification | Integrity checks and 246 qualification tests passed. |
| AG CI / addressing / networking | 7 / 22 / 16 pass; 0 / 2 / 2 explicitly unsupported. No failures/errors. |
| Formatting and default/HTML5/hardening workspace Clippy | Passed with `-D warnings`. |
| Complete default and HTML5 workspace runs with `--no-fail-fast` | Each failed only the native conformance binary target; other targets passed. |
| Parser conformance, pinned external fixtures, DOM/patch goldens, real pages, rawtext, tokenizer/tree/pipeline fuzz smoke, pipeline regressions, WPT tree builder | All passed. |
| Debug/HTML5/release/release-HTML5 builds, release networking qualification, HTML/CSS bench compilation, entity generation check | All passed. Release warnings are in unchanged HTML/CSS/runtime-policy files. |
| Deterministic runtime smoke | Existing local-file and redirected HTTP/CSS/image paths passed with accepted frames. |
| Native application smoke | Candidate started; its Borrowser window was visually inspected. Scripted URL input did not produce a loaded page, so no manual page-navigation/rendering pass is claimed. |
| Full `make ci` | Failed at workspace tests: nine native Chromium ownership/process/cleanup failures. Aggregate CI is not green. |

The exact clean detached base was independently checked with
`cargo test -p borrowser-conformance --bin borrowser-conformance native_ --locked`.
It reported 5 pass, 9 fail and 2 existing ignores. The candidate full target
reported 218 pass, the same 9 failures and 13 existing ignores. Both complete
workspace configurations reproduce exactly that failure set:

- native_capture_failure_diagnostics
- native_cleanup_diagnostic_failures
- native_lifecycle_regressions
- native_macos_bsd_ownership_states
- native_process_and_pipe_contract
- native_resource_completion_and_shutdown
- native_shutdown_uses_capture_deadline
- native_startup_exit_event_classification
- native_temporary_artifact_containment

These inherited host/platform failures are neither waived nor counted as passes.
After `make ci` stopped, both complete workspace configurations and every remaining
CI lane were executed independently. No native assertion, ignore list, timeout or
qualification gate was weakened. An earlier validation attempt with shared
cross-checkout Cargo artifacts was discarded; workspace outputs were cleaned
before the candidate validation recorded here. No later baseline build reused
those candidate artifacts.

Architecture review found no duplicate decoder, counter authority, cancellation
protocol, new dependency, full-body buffer, sensitive diagnostic output or
out-of-scope subsystem change. Private decision variant spelling was shortened
to satisfy Clippy without altering policy. Required platform CI qualification and
manual navigation qualification remain explicit limits. This candidate was left
uncommitted for owner review; it does not close AJ or certify full
HTML encoding conformance.
