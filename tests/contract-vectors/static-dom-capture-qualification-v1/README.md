# Static DOM mechanism qualification V1

These are authored qualification inputs, outside AG discovery. They are not
captured browser observations, AG fixtures, WPT assertions, or admitted evidence.
The unchanged external DOM inspector is the only serializer exercised by the
real qualification command. The same Chromium core handles these documents and
future selected-fixture collection.

`noscript.html` must produce an actual `strong` element and `NOSCRIPT-PARSED`
text node. Literal noscript source text is insufficient. `authored-effects.html`
must retain `STATIC-SENTINEL`, create no `b`/`em` mutation nodes, and record at
least one denied ancillary request. The fixture's image exercises denial; the
denial count alone does not identify a resource type.
`external-script.js` is pinned source that must never be
delivered or executed. `utf8.html` must preserve `é水🙂` despite the conflicting
meta charset.

The negative vectors require exact typed `CaptureOutcome::Rejected` values from
the shared capture core's correlated document processing:

- `redirect.html`: `StaticDomPolicyRejection::MetaRefresh` with nonempty `frame`
  and `loader`, and `destination` exactly `http://ag9g.invalid/redirected.html`.
- `child-frame.html`: `StaticDomPolicyRejection::ChildFrame` with nonempty
  `parent`, `loader`, and `child`, and `child` different from `parent`.

A rejection for the other vector, an ordinary observation, or a generic error
(including unexpected navigation, startup, protocol, or other infrastructure
failure) cannot satisfy either negative vector. Both typed rejections still
require live process/sandbox verification, quiescent late-event verification,
bounded termination/reaping, and cleanup. Cleanup or watchdog failure prevents
qualification success, even when the expected policy rejection was observed.

The executable predicates live in `qualification.rs::qualify_outcome` and
`validate_observation`; `expected.toml` documents them independently. Its
`rejection` names identify the Rust variants, `required_nonempty_fields` lists
their nonempty identity fields, and `required_distinct_fields` names the pair
that must differ. The file is source-pinned documentation, not an assertion
schema interpreted by the runner. This clarification preserves the existing
Rust predicates and qualification-suite V15 semantics.
No generated Borrowser output is
used as external truth. Successful scripted transport tests are machinery tests
only. Real qualification remains NOT ESTABLISHED on the implementation host.
