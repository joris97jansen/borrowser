# CSS parser regressions

Store minimized crashing or hanging CSS parser fuzz inputs here after triage.

Workflow:
- reproduce the failing artifact with the logged direct-binary or `cargo fuzz run` command,
- minimize the input while preserving the failure,
- commit the minimized input here with a stable descriptive name, and
- replay the committed corpus through
  `make test-css-parser-fuzz-corpus`.

Replay a single artifact through the actual fuzz target:

```sh
cargo fuzz run css_parser fuzz/regressions/css_parser/<name>
```
