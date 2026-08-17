use std::ops::{Add, AddAssign};

/// CSS selector specificity tuple `(a, b, c)`.
///
/// `a`: id selectors
/// `b`: class selectors, attribute selectors, and pseudo-classes
/// `c`: type selectors and pseudo-elements
///
/// The field declaration order is semantic: the derived `Ord` implementation
/// compares the tuple lexicographically as A, then B, then C. Keep these
/// fields in that order unless the comparison implementation is updated with
/// equivalent explicit semantics.
///
/// Saturating arithmetic is used so hostile input cannot overflow specificity
/// accounting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Ord, PartialOrd)]
pub struct Specificity {
    a: u16,
    b: u16,
    c: u16,
}

impl Specificity {
    pub const ZERO: Self = Self { a: 0, b: 0, c: 0 };
    pub const A: Self = Self { a: 1, b: 0, c: 0 };
    pub const B: Self = Self { a: 0, b: 1, c: 0 };
    pub const C: Self = Self { a: 0, b: 0, c: 1 };

    pub const fn new(a: u16, b: u16, c: u16) -> Self {
        Self { a, b, c }
    }

    pub const fn a(self) -> u16 {
        self.a
    }

    pub const fn b(self) -> u16 {
        self.b
    }

    pub const fn c(self) -> u16 {
        self.c
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            a: self.a.saturating_add(other.a),
            b: self.b.saturating_add(other.b),
            c: self.c.saturating_add(other.c),
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
