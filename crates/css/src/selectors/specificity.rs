use std::ops::{Add, AddAssign};

/// CSS selector specificity tuple `(a, b, c)`.
///
/// `a`: id selectors
/// `b`: class selectors and attribute selectors
/// `c`: type selectors
///
/// Saturating arithmetic is used so hostile input cannot overflow specificity
/// accounting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Ord, PartialOrd)]
pub struct Specificity {
    ids: u16,
    classes: u16,
    types: u16,
}

impl Specificity {
    pub const ZERO: Self = Self {
        ids: 0,
        classes: 0,
        types: 0,
    };
    pub const ID: Self = Self {
        ids: 1,
        classes: 0,
        types: 0,
    };
    pub const CLASS: Self = Self {
        ids: 0,
        classes: 1,
        types: 0,
    };
    pub const TYPE: Self = Self {
        ids: 0,
        classes: 0,
        types: 1,
    };

    pub const fn new(ids: u16, classes: u16, types: u16) -> Self {
        Self {
            ids,
            classes,
            types,
        }
    }

    pub fn ids(self) -> u16 {
        self.ids
    }

    pub fn classes(self) -> u16 {
        self.classes
    }

    pub fn types(self) -> u16 {
        self.types
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            ids: self.ids.saturating_add(other.ids),
            classes: self.classes.saturating_add(other.classes),
            types: self.types.saturating_add(other.types),
        }
    }
}

impl Add for Specificity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl AddAssign for Specificity {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.saturating_add(rhs);
    }
}
