# HTML5 Tree Builder Synthetic-Token Regressions

Store minimized crashing or hanging synthetic-token fuzz inputs here after triage.

Workflow:
- reproduce the failing artifact with the logged direct-binary or `cargo fuzz run` command,
- minimize it while preserving the failure,
- commit the minimized bytes here with a descriptive name, and
- replay the committed corpus through
  `html5::tree_builder::fuzz::tests::corpus::replay_committed_tree_builder_token_corpus_deterministically`.

Synthetic tree-builder inputs use an explicit decoder-version contract:

- unmarked inputs are decoder V1 and retain the exact 30-name catalog and
  modulo mapping from before the AE9b select extension;
- an exact `TB-FUZZ-V2\n` prefix is decoder metadata, is not emitted as a
  token, and selects the V2 catalog containing the AE9b select-family names;
- truncated or unknown marker-like prefixes are V1 token bytes.

All committed corpus and regression entries predating the AE9b select extension
remain unmodified V1 inputs. The only committed V2 input is the exact path
`fuzz/regressions/html5_tree_builder_tokens/select-special-barrier`; a matching
basename in any other directory is not the documented exception. It is a Local
WHATWG-derived fixture; not an upstream WPT or html5lib-tests import. Its token
bytes cover malformed select/option/div/optgroup end-tag recovery under WHATWG
commit `88ae68cb961651f0f92c5d2046049f53ecdfc6cf`.

The V2 prefix belongs to raw-input accounting: its bytes count toward
`max_input_bytes`, appear in `TreeBuilderFuzzSummary::input_bytes`, and
participate in raw-input fuzz-seed derivation. It is removed only at the
decoded-stream boundary, so it contributes no generated tokens, attributes,
string bytes, decoded-token processing steps, or emitted synthetic token.
Including framing in the raw cap keeps that bound strict; including it in seed
derivation domain-separates V1 and V2 raw input identities.

Decoder versioning preserves the meaning of corpus bytes; it is distinct from
the global `FuzzDigest` schema. AE11 owns comprehensive future-affecting-state
digest coverage.
