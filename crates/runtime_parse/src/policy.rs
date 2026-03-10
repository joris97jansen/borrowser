use log::warn;
use std::time::Duration;

pub(crate) const DEFAULT_TICK: Duration = Duration::from_millis(180);
pub(crate) const DEBUG_LARGE_BUFFER_BYTES: usize = 1_048_576;
// Cap retained patch buffer capacity to avoid pinning memory after a spike.
pub(crate) const MAX_PATCH_BUFFER_RETAIN: usize = 4_096;
pub(crate) const MIN_PATCH_BUFFER_RETAIN: usize = 256;
pub(crate) const EST_PATCH_BYTES: usize = 64;

/// Retain capacity for patch vectors to reduce allocator churn while capping memory.
pub(crate) fn patch_buffer_retain_target(
    patch_threshold: Option<usize>,
    patch_byte_threshold: Option<usize>,
) -> usize {
    let base = if let Some(threshold) = patch_threshold {
        threshold.saturating_mul(2).max(MIN_PATCH_BUFFER_RETAIN)
    } else if let Some(bytes) = patch_byte_threshold {
        // Approximate patch count from bytes; conservative to avoid churn.
        (bytes / EST_PATCH_BYTES).max(MIN_PATCH_BUFFER_RETAIN)
    } else {
        MIN_PATCH_BUFFER_RETAIN
    };
    base.min(MAX_PATCH_BUFFER_RETAIN)
}

/// Preview flush strategy for incremental parse.
///
/// The policy is evaluated when new input arrives. A flush occurs if any enabled
/// threshold is met and there are pending patches. Thresholds include time, input
/// tokens/bytes, and buffered patch count/bytes. Time-based checks are activity
/// driven: ticks are evaluated only when input arrives (no background timer). If
/// input stalls, pending patches are flushed on `ParseHtmlDone`. Boundedness
/// assumes continued input or an eventual `ParseHtmlDone`.
#[derive(Clone, Copy, Debug)]
pub struct PreviewPolicy {
    pub tick: Duration,
    pub token_threshold: Option<usize>,
    pub byte_threshold: Option<usize>,
    pub patch_threshold: Option<usize>,
    pub patch_byte_threshold: Option<usize>,
}

impl Default for PreviewPolicy {
    fn default() -> Self {
        Self {
            tick: DEFAULT_TICK,
            token_threshold: None,
            byte_threshold: None,
            patch_threshold: None,
            patch_byte_threshold: None,
        }
    }
}

impl PreviewPolicy {
    pub(crate) fn is_bounded(&self) -> bool {
        self.tick != Duration::ZERO
            || self.token_threshold.is_some()
            || self.byte_threshold.is_some()
            || self.patch_threshold.is_some()
            || self.patch_byte_threshold.is_some()
    }

    pub(crate) fn ensure_bounded(mut self) -> Self {
        if !self.is_bounded() {
            self.tick = DEFAULT_TICK;
        }
        self
    }

    pub(crate) fn should_flush(
        &self,
        elapsed: Duration,
        pending_tokens: usize,
        pending_bytes: usize,
        pending_patches: usize,
        pending_patch_bytes: usize,
    ) -> bool {
        if pending_patches == 0 {
            return false;
        }
        if self.tick != Duration::ZERO && elapsed >= self.tick {
            return true;
        }
        if let Some(threshold) = self.token_threshold
            && pending_tokens >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.byte_threshold
            && pending_bytes >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.patch_threshold
            && pending_patches >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.patch_byte_threshold
            && pending_patch_bytes >= threshold
        {
            return true;
        }
        false
    }
}

pub(crate) fn maybe_log_large_buffer(total_bytes: usize, logged_large_buffer: &mut bool) {
    #[cfg(debug_assertions)]
    {
        if !*logged_large_buffer && total_bytes >= DEBUG_LARGE_BUFFER_BYTES {
            warn!(
                target: "runtime_parse",
                "large buffer ({} bytes), incremental parse active",
                total_bytes
            );
            *logged_large_buffer = true;
        }
    }
}
