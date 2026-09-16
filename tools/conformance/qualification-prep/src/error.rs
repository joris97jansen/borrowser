use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Invalid(&'static str),
    Io(String),
    Cleanup,
    Timeout,
    Unsupported,
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "preparation failed: {self:?}")
    }
}
impl std::error::Error for Error {}
pub fn require(ok: bool, what: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(what))
    }
}
/// Cleanup is always attempted by callers, and takes precedence over operation failure.
pub fn after_cleanup<T>(value: Result<T>, cleanup: Result<()>) -> Result<T> {
    cleanup.map_err(|_| Error::Cleanup)?;
    value
}
