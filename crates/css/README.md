# css crate: performance harness

The CSS crate uses deterministic fixtures for parser, selector-matching,
cascade, and computed-style performance work. The default test lane includes
smoke guards that assert stable structural counts and safe computed-style reuse
behavior without relying on wall-clock timing.

Run CSS tests and smoke guards:

```bash
cargo test -p css
```

Run heavier deterministic perf guards:

```bash
cargo test -p css --features perf-tests
```

Compile the Criterion benchmark harness:

```bash
cargo bench -p css --bench css_bench --no-run
```

Run the Criterion benchmark harness:

```bash
cargo bench -p css --bench css_bench
```

Run allocation guards:

```bash
cargo test -p css --test alloc_guards --features count-alloc
```

The allocation guards are opt-in because they install a test-local global
allocator. They measure allocation events, allocation growth bytes, and realloc
events for representative parse and style-resolution workloads.

Current U6 scope:

- Benchmarks cover CSS parsing, selector matching, and integrated style
  resolution.
- Smoke perf guards are deterministic and run under normal `cargo test -p css`.
- Heavy guards are deterministic and feature-gated behind `perf-tests`.
- Allocation guards are feature-gated behind `count-alloc`.
- Thresholds are intentionally conservative bounds, not browser-grade
  optimization targets.

The smoke and allocation guards are regression tripwires, not final browser
performance targets. Criterion results are the timing source of truth and should
be compared with local or CI baselines when evaluating performance-sensitive
changes.
