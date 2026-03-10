use core_types::{DomHandle, DomVersion};
use html::PatchKey;

#[derive(Debug)]
pub enum DomPatchError {
    UnknownHandle(DomHandle),
    DuplicateHandle(DomHandle),
    VersionMismatch {
        expected: DomVersion,
        got: DomVersion,
    },
    NonMonotonicVersion {
        from: DomVersion,
        to: DomVersion,
    },
    Protocol(&'static str),
    InvalidKey(PatchKey),
    DuplicateKey(PatchKey),
    MissingKey(PatchKey),
    WrongNodeKind {
        key: PatchKey,
        expected: &'static str,
        actual: &'static str,
    },
    InvalidParent(PatchKey),
    MoveNotSupported {
        key: PatchKey,
    },
    InvalidSibling {
        parent: PatchKey,
        before: PatchKey,
    },
    CycleDetected {
        parent: PatchKey,
        child: PatchKey,
    },
    MissingRoot,
    UnsupportedPatch(&'static str),
}
