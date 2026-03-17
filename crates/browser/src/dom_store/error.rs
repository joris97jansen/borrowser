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
    // Retained for legacy/non-HTML5-capable appliers that may still reject
    // reparenting outright. Strict HTML5-capable appliers should instead
    // support legal moves and use `IllegalMove` only for forbidden cases.
    MoveNotSupported {
        key: PatchKey,
    },
    IllegalMove {
        key: PatchKey,
        reason: &'static str,
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
