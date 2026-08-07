mod disposition;
mod execution;
mod failure_spelling;
mod load;
mod mismatch;
mod model;
mod runner;
mod schema;
mod validate;

pub use load::{
    DeliveryValidationError, FixtureLoadError, FixtureLoadErrorKind, FixturePlanningInvariant,
    FixtureRepository, FixtureRepositoryPolicy, discover_and_load,
};
pub(crate) use model::ExpectationSurface;
pub use model::{
    DeliveryName, DispositionEvaluation, FixtureDeliveryRunReport, FixtureId, FixtureRunReport,
    FixtureSourceKind, ParserTargetKind, ScriptingMode, SnapshotPath,
};
pub use runner::{
    FixtureCorpusFailure, FixtureCorpusRunError, FixtureRunError, run_fixture, run_fixture_corpus,
};
pub use schema::*;
pub use validate::ValidatedFixtureSpec;

#[cfg(test)]
mod tests;
