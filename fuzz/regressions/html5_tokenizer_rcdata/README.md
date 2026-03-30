# HTML5 Tokenizer RCDATA Regression Inputs

Store minimized crashing or hanging RCDATA fuzz inputs here after triage.

Workflow:
- download the failure artifact from CI,
- reproduce it locally with the logged direct-binary or `cargo fuzz run` command,
- minimize and rename it descriptively, and
- commit it into this directory.

Committed regression entries are replayed by the deterministic RCDATA
tokenizer fuzz replay tests alongside the seed corpus.
