//! Frame-local identifiers for generated box-tree structures.

/// Stable index for a generated layout box in a frame-local box tree.
///
/// Box IDs are deterministic for a fixed style tree and generation environment:
/// nodes are assigned in preorder as they are generated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoxId(pub(in crate::box_tree) usize);

impl BoxId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Stable frame-local identity of the box that provides a containing block.
///
/// W5 models containing blocks as relationships between generated boxes. The
/// ID wraps the establishing `BoxId` so future layout modes can distinguish
/// "this box" from "the containing block this box resolves against" without
/// relying on raw parent traversal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContainingBlockId(pub(in crate::box_tree) BoxId);

impl ContainingBlockId {
    pub fn box_id(self) -> BoxId {
        self.0
    }

    pub fn index(self) -> usize {
        self.0.index()
    }
}

/// Stable frame-local identity of the box that provides the containing block
/// for CSS positioned layout.
///
/// This is separate from `ContainingBlockId`: normal-flow sizing and positioned
/// layout can resolve against different generated ancestors. Y5 records that
/// relationship before final positioned geometry is implemented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PositionedContainingBlockId(pub(in crate::box_tree) BoxId);

impl PositionedContainingBlockId {
    pub fn box_id(self) -> BoxId {
        self.0
    }

    pub fn index(self) -> usize {
        self.0.index()
    }
}

/// Stable frame-local identity of a formatting context root.
///
/// W6 models the supported normal-flow block formatting scope as generated-box
/// identity. This is intentionally separate from DOM parentage and from
/// containing-block identity so later layout modes can refine context roots
/// without changing the source tree contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormattingContextId(pub(in crate::box_tree) BoxId);

impl FormattingContextId {
    pub fn box_id(self) -> BoxId {
        self.0
    }

    pub fn index(self) -> usize {
        self.0.index()
    }
}

/// Stable frame-local identity of an inline formatting context root.
///
/// W7 models inline formatting contexts as line-building scopes established by
/// generated boxes. This is intentionally separate from block formatting
/// context identity: an inline-block can participate atomically in one inline
/// context while establishing another for its descendants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InlineFormattingContextId(pub(in crate::box_tree) BoxId);

impl InlineFormattingContextId {
    pub fn box_id(self) -> BoxId {
        self.0
    }

    pub fn index(self) -> usize {
        self.0.index()
    }
}
