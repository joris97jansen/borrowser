# HTML5 Tokenizer Script-Data Regression Inputs

Store minimized crashing or hanging script-data fuzz inputs here after triage.

Workflow:
- download the failure artifact from CI,
- reproduce it locally with the logged direct-binary or `cargo fuzz run` command,
- minimize and rename it descriptively, and
- commit it into this directory.

Committed regression entries are replayed by the deterministic script-data
tokenizer fuzz replay tests alongside the seed corpus.
