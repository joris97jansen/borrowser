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

AF4c also includes a focused selector-comparison guard. Parsing, DOM and
selector construction, selector-DOM projection, context construction, and any
match-result vectors occur before measurement. The measured region repeatedly
calls `matches_compound_selector(...)`, observably consumes the successful
match count, and requires exactly zero allocation events, allocated bytes, and
reallocation events. This directly protects the borrowed host-language name,
ID/class value, and attribute-operator hot path rather than relying on the
broader whole-style-resolution thresholds.

Current U6 scope:

- Benchmarks cover CSS parsing, selector matching, and integrated style
  resolution.
- Smoke perf guards are deterministic and run under normal `cargo test -p css`.
- Heavy guards are deterministic and feature-gated behind `perf-tests`.
- Allocation guards are feature-gated behind `count-alloc`.
- Host-language selector comparison has an exact-zero focused allocation guard;
  broader parse/style guards retain conservative workload-relative thresholds.
- Thresholds are intentionally conservative bounds, not browser-grade
  optimization targets.

The smoke and allocation guards are regression tripwires, not final browser
performance targets. Criterion results are the timing source of truth and should
be compared with local or CI baselines when evaluating performance-sensitive
changes.
