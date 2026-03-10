use std::time::Duration;

use crate::PreviewPolicy;
use crate::policy::{MAX_PATCH_BUFFER_RETAIN, MIN_PATCH_BUFFER_RETAIN, patch_buffer_retain_target};

#[test]
fn preview_policy_flushes_on_thresholds() {
    let policy = PreviewPolicy {
        tick: Duration::from_millis(100),
        token_threshold: Some(10),
        byte_threshold: Some(256),
        patch_threshold: Some(5),
        patch_byte_threshold: Some(64),
    };

    assert!(
        !policy.should_flush(Duration::from_millis(50), 0, 0, 0, 0),
        "should not flush before thresholds"
    );
    assert!(
        policy.should_flush(Duration::from_millis(150), 0, 0, 1, 0),
        "should flush on time"
    );
    assert!(
        policy.should_flush(Duration::from_millis(10), 10, 0, 1, 0),
        "should flush on token threshold"
    );
    assert!(
        policy.should_flush(Duration::from_millis(10), 0, 256, 1, 0),
        "should flush on byte threshold"
    );
    assert!(
        policy.should_flush(Duration::from_millis(10), 0, 0, 5, 0),
        "should flush on patch threshold"
    );
    assert!(
        policy.should_flush(Duration::from_millis(10), 0, 0, 1, 64),
        "should flush on patch byte threshold"
    );
    assert!(
        !policy.should_flush(Duration::from_millis(150), 10, 256, 0, 64),
        "should not flush without pending patches"
    );
}

#[test]
fn preview_policy_unbounded_is_clamped() {
    let policy = PreviewPolicy {
        tick: Duration::ZERO,
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: None,
        patch_byte_threshold: None,
    };
    let bounded = policy.ensure_bounded();
    assert!(
        bounded.is_bounded(),
        "expected unbounded policy to be clamped"
    );
    assert!(
        bounded.tick != Duration::ZERO,
        "expected clamped policy to restore a tick"
    );
}

#[test]
fn patch_buffer_retain_target_clamps_to_max() {
    let huge = MAX_PATCH_BUFFER_RETAIN.saturating_mul(10);
    let retain = patch_buffer_retain_target(Some(huge), None);
    assert_eq!(
        retain, MAX_PATCH_BUFFER_RETAIN,
        "expected retain target to clamp to max"
    );
}

#[test]
fn patch_buffer_retain_target_has_floor() {
    let retain = patch_buffer_retain_target(None, None);
    assert_eq!(retain, MIN_PATCH_BUFFER_RETAIN);
}
