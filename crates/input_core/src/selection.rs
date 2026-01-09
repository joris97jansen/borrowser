//! Text selection representation.

/// Represents a text selection as a byte range.
///
/// The range is always normalized such that `start <= end`.
/// Both `start` and `end` are byte offsets into a UTF-8 string and
/// are guaranteed to be on valid character boundaries when produced
/// by [`InputValueStore`](crate::InputValueStore) methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    /// Start byte offset of the selection (inclusive).
    pub start: usize,
    /// End byte offset of the selection (exclusive).
    pub end: usize,
}

impl SelectionRange {
    /// Create a new selection range.
    ///
    /// The range is automatically normalized so `start <= end`.
    #[inline]
    pub fn new(a: usize, b: usize) -> Self {
        Self {
            start: a.min(b),
            end: a.max(b),
        }
    }

    /// Returns `true` if the selection is empty (zero-width).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the length of the selection in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns the selected substring from the given value.
    ///
    /// # Panics
    ///
    /// Panics if `start` or `end` are out of bounds or not on character boundaries.
    #[inline]
    pub fn slice<'a>(&self, value: &'a str) -> &'a str {
        &value[self.start..self.end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_range_normalizes() {
        let range = SelectionRange::new(10, 5);
        assert_eq!(range.start, 5);
        assert_eq!(range.end, 10);
    }

    #[test]
    fn selection_range_len() {
        let range = SelectionRange::new(2, 7);
        assert_eq!(range.len(), 5);
    }

    #[test]
    fn selection_range_is_empty() {
        let empty = SelectionRange::new(3, 3);
        assert!(empty.is_empty());

        let non_empty = SelectionRange::new(3, 5);
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn selection_range_slice() {
        let text = "hello world";
        let range = SelectionRange::new(0, 5);
        assert_eq!(range.slice(text), "hello");
    }
}
