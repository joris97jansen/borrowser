/// Current supported layout participation category for a generated layout box.
///
/// This is not yet the full CSS box-generation model. Milestone W will expand
/// this toward explicit root, anonymous, marker, and formatting-context-aware
/// box roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    Block,
    Inline,
    InlineBlock,
    ReplacedInline,
    // Future: Root, AnonymousBlock, AnonymousInline, Marker, ListItem, etc.
}

/// What kind of list marker this block has, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMarker {
    /// Bullet for unordered lists (<ul><li>).
    Unordered,
    /// Numbered marker for ordered lists (<ol><li>), 1-based.
    Ordered(u32),
}
