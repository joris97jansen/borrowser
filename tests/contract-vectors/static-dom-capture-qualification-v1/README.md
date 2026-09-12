# Static DOM mechanism qualification V1

These are authored qualification inputs, outside AG discovery. They are not
captured browser observations, AG fixtures, WPT assertions, or admitted evidence.
The unchanged external DOM inspector is the only serializer exercised by the
real qualification command. The same Chromium core handles these documents and
future selected-fixture collection.

`noscript.html` must produce an actual `strong` element and `NOSCRIPT-PARSED`
text node. Literal noscript source text is insufficient. `authored-effects.html`
must retain `STATIC-SENTINEL`, create no `b`/`em` mutation nodes, and record a
denied image request. `external-script.js` is pinned source that must never be
delivered or executed. `utf8.html` must preserve `é水🙂` despite the conflicting
meta charset. `redirect.html` and `child-frame.html` must fail with an unexpected
navigation/target result, not merely any infrastructure error.

The exact predicates live in the reviewed qualification implementation and
`expected.toml` documents them independently. No generated Borrowser output is
used as external truth. Successful scripted transport tests are machinery tests
only. Real qualification remains NOT ESTABLISHED on the implementation host.
