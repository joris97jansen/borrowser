use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchValidationError {
    context: &'static str,
    detail: String,
}

impl PatchValidationError {
    pub(crate) fn new(context: &'static str, detail: impl Into<String>) -> Self {
        Self {
            context,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for PatchValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context, self.detail)
    }
}

impl std::error::Error for PatchValidationError {}

pub(crate) type ArenaResult<T> = Result<T, PatchValidationError>;
