mod disposition;
mod evaluation;
mod execution;
mod failure_spelling;
mod load;
mod mismatch;
mod model;
mod runner;
mod schema;
mod validate;

pub use evaluation::{
    FixtureAttemptState, FixtureDispositionEvaluation, FixtureEvaluation,
    FixtureExecutionFailureCategory, FixtureObservedOutcome, IncompleteObservationReason,
    ParserObservationSerializationError, SerializedParserObservation, StableFixtureIdentity,
};
pub use load::{
    DeliveryValidationError, FixtureLoadError, FixtureLoadErrorKind, FixturePlanningInvariant,
    FixtureRepository, FixtureRepositoryPolicy, discover_and_load,
};
pub use model::{
    DeclaredExpectation, DeliveryName, DispositionEvaluation, ExpectationSurface,
    FixtureDeliveryRunReport, FixtureDispositionKind, FixtureId, FixtureRunReport,
    FixtureSourceKind, ParseErrorExpectationStrength, ParserFixtureExecutionModel,
    ParserTargetKind, ScriptingMode, SnapshotPath,
};
pub use runner::{
    FixtureCorpusFailure, FixtureCorpusRunError, FixtureRunError, evaluate_fixture, run_fixture,
    run_fixture_corpus,
};
pub use schema::*;
pub use validate::ValidatedFixtureSpec;

#[cfg(test)]
mod tests;
