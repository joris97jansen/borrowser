use crate::{LayoutBox, Rectangle, ReplacedKind};
use css::ComputedStyle;
use html::internal::Id;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdvanceRect(Rectangle);

impl AdvanceRect {
    #[inline]
    pub(super) fn new(rect: Rectangle) -> Self {
        Self(rect)
    }

    #[inline]
    pub fn rect(self) -> Rectangle {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintRect(Rectangle);

impl PaintRect {
    #[inline]
    pub(super) fn new(rect: Rectangle) -> Self {
        Self(rect)
    }

    #[inline]
    pub fn rect(self) -> Rectangle {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineActionKind {
    Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineAction {
    pub target: Id,
    pub kind: InlineActionKind,
    pub href: Option<Arc<str>>,
}

/// The logical content carried by a line fragment.
/// - `Text` is inline text
/// - `Box` is inline-level replaced/box content (e.g., inline-block)
pub enum InlineFragment<'a> {
    Text {
        text: String,
        style: &'a ComputedStyle,
        action: Option<InlineAction>,
    },
    Box {
        /// Style of the inline box (for color, etc.).
        style: &'a ComputedStyle,
        action: Option<InlineAction>,
        /// Layout box for this inline-level box, if we have one.
        /// - `Some(..)` in the painting path
        /// - `None` in the height computation path
        layout: Option<&'a LayoutBox<'a>>,
    },
    Replaced {
        style: &'a ComputedStyle,
        kind: ReplacedKind,
        action: Option<InlineAction>,
        layout: Option<&'a LayoutBox<'a>>, // usually None; future-proof (e.g. <button>)
    },
}

// One fragment of text within a line (later this can be per <span>, <a>, etc.)
pub struct LineFragment<'a> {
    pub kind: InlineFragment<'a>,
    /// Rect used for inline layout advance (typically margin-box for inline boxes).
    pub advance_rect: AdvanceRect,
    /// Rect used for painting/hit-testing (typically border-box for inline boxes).
    pub paint_rect: PaintRect,
    /// Optional mapping back to a source text byte range (start, end).
    ///
    /// This is `None` for DOM-driven inline layout, but can be populated by
    /// host controls like `<textarea>` that lay out their own internal text.
    pub source_range: Option<(usize, usize)>,
    /// Distance from the fragment top edge to its baseline (CSS px).
    pub ascent: f32,
    /// Distance from the baseline to the fragment bottom edge (CSS px).
    pub descent: f32,
    /// Additional baseline shift applied during final positioning (CSS px).
    ///
    /// This is a forward-compatible hook for CSS `vertical-align` (e.g. `super`,
    /// `sub`, `middle`, `top`, explicit lengths, etc).
    ///
    /// The fragment baseline in layout coordinates is
    /// `advance_rect.rect().y + ascent + baseline_shift`.
    pub baseline_shift: f32,
}

// One line box: a horizontal slice of inline content.
pub struct LineBox<'a> {
    pub fragments: Vec<LineFragment<'a>>,
    pub rect: Rectangle,
    /// Line baseline in layout coordinates (CSS px).
    pub baseline: f32,
    /// Optional mapping back to the source text byte range (start, end) covered by this line.
    pub source_range: Option<(usize, usize)>,
}
