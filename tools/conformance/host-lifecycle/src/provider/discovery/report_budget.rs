//! One platform-independent logical allowance for all derived report metadata.
//! This counts owned logical payload, not allocator capacity or machine pointers.
pub const REPORT_METADATA_BYTES: usize = 64 << 10;
pub const REPORT_HEADER_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportBudget {
    used: usize,
    exhausted: bool,
}
impl Default for ReportBudget {
    fn default() -> Self {
        Self {
            used: REPORT_HEADER_BYTES,
            exhausted: false,
        }
    }
}
impl ReportBudget {
    pub fn used(&self) -> usize {
        self.used
    }
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }
    pub(crate) fn charge(&mut self, bytes: usize) -> bool {
        if self.exhausted || bytes > REPORT_METADATA_BYTES - self.used {
            self.exhausted = true;
            return false;
        }
        self.used += bytes;
        true
    }
}
