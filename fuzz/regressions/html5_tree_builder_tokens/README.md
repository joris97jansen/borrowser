# HTML5 Tree Builder Synthetic-Token Regressions

Store minimized crashing or hanging synthetic-token fuzz inputs here after triage.

Workflow:
- reproduce the failing artifact with the logged direct-binary or `cargo fuzz run` command,
- minimize it while preserving the failure,
- commit the minimized bytes here with a descriptive name, and
- replay the committed corpus through
  `html5::tree_builder::fuzz::tests::corpus::replay_committed_tree_builder_token_corpus_deterministically`.
