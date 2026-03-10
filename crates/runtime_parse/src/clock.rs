use std::time::Instant;

pub(crate) trait PreviewClock: Send {
    fn now(&self) -> Instant;
}

pub(crate) struct SystemClock;

impl PreviewClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
