#[cfg(feature = "perf-tests")]
use super::super::tokenize;
#[cfg(feature = "perf-tests")]
use std::time::{Duration, Instant};

#[cfg(feature = "perf-tests")]
#[test]
fn tokenize_scales_roughly_linearly_on_repeated_tags() {
    fn build_input(repeats: usize) -> String {
        let mut input = String::new();
        for _ in 0..repeats {
            input.push_str("<a></a>");
        }
        input
    }

    fn measure_total(input: &str) -> Duration {
        let _ = tokenize(input);
        let mut total = Duration::ZERO;
        for _ in 0..5 {
            let start = Instant::now();
            let _ = tokenize(input);
            total += start.elapsed();
        }
        total
    }

    let small = build_input(5_000);
    let large = build_input(20_000);

    let t_small = measure_total(&small);
    let t_large = measure_total(&large);
    assert!(!t_small.is_zero(), "timer resolution too coarse for test");
    assert!(
        t_large <= t_small.saturating_mul(12),
        "expected near-linear scaling; t_small={t_small:?} t_large={t_large:?}"
    );
}

#[cfg(feature = "perf-tests")]
#[test]
fn tokenize_scales_roughly_linearly_on_comment_scan() {
    fn build_input(repeats: usize, body_len: usize) -> String {
        let mut input = String::new();
        for _ in 0..repeats {
            input.push_str("<!--");
            input.extend(std::iter::repeat_n('-', body_len));
            input.push('x');
            input.push_str("-->");
        }
        input
    }

    fn measure_total(input: &str) -> Duration {
        let _ = tokenize(input);
        let mut total = Duration::ZERO;
        for _ in 0..5 {
            let start = Instant::now();
            let _ = tokenize(input);
            total += start.elapsed();
        }
        total
    }

    let small = build_input(500, 400);
    let large = build_input(2_000, 400);

    let t_small = measure_total(&small);
    let t_large = measure_total(&large);
    assert!(!t_small.is_zero(), "timer resolution too coarse for test");
    assert!(
        t_large <= t_small.saturating_mul(12),
        "expected near-linear comment scan; t_small={t_small:?} t_large={t_large:?}"
    );
}
