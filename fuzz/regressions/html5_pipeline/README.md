# HTML5 End-to-End Pipeline Regressions

Store minimized crashing or hanging byte-stream fuzz inputs here after triage.

Workflow:
- reproduce the failing artifact with the logged direct-binary or `cargo fuzz run` command,
- minimize it while preserving the failure,
- commit the minimized bytes here with a descriptive name, and
- replay the committed corpus through
  `html5::fuzz::tests::corpus::replay_committed_html5_pipeline_corpus_deterministically`.
