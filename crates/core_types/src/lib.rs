pub type TabId = u64;
pub type RequestId = u64;

/// Stable identity for a live document owned by a parse session.
///
/// Handles are created by the owning subsystem; `0` is reserved and must not be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DomHandle(pub u64);

/// Monotonic version for a document identified by a `DomHandle`.
///
/// The initial version is `0`; the first mutation produces `1` and versions
/// are expected to increment by exactly 1 per patch in the current model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DomVersion(pub u64);

impl DomVersion {
    /// Initial version for a newly created document.
    pub const INITIAL: DomVersion = DomVersion(0);

    /// Next version in the patch stream.
    pub fn next(self) -> DomVersion {
        DomVersion(self.0 + 1)
    }
}

/// Patch sequence id; alias of `DomVersion` to make intent explicit.
///
/// A patch always applies to a specific `(DomHandle, from_version)` and
/// produces a `to_version`.
///
/// In v5.1 the patch sequence matches DOM versions; this may diverge later to
/// support transport-level concerns (replay, bundling, retransmit).
pub type PatchSeq = DomVersion;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrowserInput {
    pub enter_pressed: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum ResourceKind {
    Html,
    Css,
    Image,
}
