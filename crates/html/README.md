# html crate: performance harness

## Benchmarks

This crate uses Criterion for statistically robust micro-benchmarks that are
repeatable locally with stable methodology.

Run all HTML benches:

```sh
cargo bench -p html --features test-harness
```

Run only the HTML bench harness:

```sh
cargo bench -p html --bench html_bench --features test-harness
```

## Perf regression guards (CI)

Deterministic regression guards run under `cargo test` and validate token
counts, node counts, and token/byte ratios. The heavier cases are gated behind
the `perf-tests` feature.

Run locally:

```sh
cargo test -p html --all-targets
```

Run the heavier perf guards (intended for CI):

```sh
cargo test -p html --all-targets --features perf-tests
```

Note: `perf-tests` includes timing-based regression checks. Prefer running these
in a dedicated nightly perf workflow (optionally with `RUST_TEST_THREADS=1`)
rather than on every PR.

## Allocation guards (opt-in)

Allocation-count tests live in a dedicated integration test binary to keep
allocator overrides scoped. Enable explicitly:

```sh
cargo test -p html --test alloc_guards --features count-alloc
```
