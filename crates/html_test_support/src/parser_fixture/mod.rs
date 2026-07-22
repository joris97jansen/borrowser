mod disposition;
mod load;
mod model;
mod runner;
mod schema;
mod validate;

pub use load::{
    FixtureLoadError, FixtureLoadErrorKind, FixtureRepository, FixtureRepositoryPolicy,
    discover_and_load,
};
pub use model::{
    DeliveryName, DispositionEvaluation, FixtureId, FixtureRunReport, FixtureSourceKind,
    ParserTargetKind, ScriptingMode, SnapshotPath,
};
pub use runner::{
    FixtureCorpusFailure, FixtureCorpusRunError, FixtureRunError, run_fixture, run_fixture_corpus,
};
pub use schema::*;
pub use validate::ValidatedFixtureSpec;

#[cfg(test)]
mod tests;
