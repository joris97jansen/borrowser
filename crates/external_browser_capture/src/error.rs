use std::fmt;

/// Closed local failures; never projected into aggregate conformance outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureError {
    UnsupportedHost,
    UnavailableMode,
    Arguments,
    Read,
    Schema,
    NonCanonical,
    Field,
    Limit,
    Allocation,
    Path,
    Digest,
    Source,
    Distribution,
    Mode,
    Capability,
    Symlink,
    Isolation,
    ProcessIdentity,
    Sandbox,
    Launch,
    Protocol,
    Framing,
    Acknowledgement,
    UnexpectedEvent,
    UnexpectedRequest,
    UnexpectedNavigation,
    DocumentIdentity,
    RealmIdentity,
    ScriptingControl,
    Completion,
    Inspection,
    Artifact,
    Qualification,
    Deadline,
    Cleanup,
    Publication,
}
pub type Result<T> = std::result::Result<T, CaptureError>;
impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "external capture rejected: {self:?}")
    }
}
impl std::error::Error for CaptureError {}
