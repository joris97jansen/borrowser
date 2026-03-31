mod fixtures;
mod model;

pub use fixtures::fixtures;
pub use model::{AllowedFailure, Expectation, FixtureKind, GoldenFixture, Invariant};

#[cfg(all(test, feature = "html5"))]
mod tests;
