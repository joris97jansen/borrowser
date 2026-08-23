mod declarations;
mod order;
mod priority;
mod properties;
mod resolved_style;
mod rules;
mod serialize;
mod snapshot;
mod sources;
mod winners;

pub use declarations::{
    CascadeDeclarationApplicability, CascadeDeclarationInput, CascadeDeclarationProperty,
    CascadeSpecifiedValue,
};
pub use order::{
    DeclarationOrder, DeclarationSourceIndex, RawRuleIndex, SourceCoordinateError,
    StyleRulePosition, StylesheetOrder, StylesheetRuleOrder, StylesheetSourceId,
    StylesheetSourceIdError,
};
pub use priority::{
    CascadeDeclarationPrecedence, CascadeImportance, CascadeOrigin, CascadeOriginBand,
    CascadePriority, CurrentScopeCascadePriorityBand,
};
pub use properties::{
    CascadeInheritance, CascadePropertyId, CascadePropertyInvalidationImpact,
    CascadePropertyLengthSignPolicy, CascadePropertyMetadata, CascadePropertyRegistration,
    CascadePropertyRegistry, CascadeShorthandId, CascadeShorthandRegistration,
    CascadeShorthandRegistry, InitialStyleValue, cascade_property_registry,
    cascade_property_registry_metadata_debug_snapshot, cascade_shorthand_registry,
};
#[cfg(test)]
pub use resolved_style::resolve_cascade_style_from_rule_inputs;
pub use resolved_style::{
    CssWideResolvedSource, ResolvedStyle, ResolvedStyleBuildError, ResolvedStyleBuilder,
    ResolvedStyleEntry, ResolvedValueSource, resolve_cascade_style, resolve_initial_style,
};
pub use rules::{
    CascadeRuleInput, CascadeRuleInputBuildError, InlineStyleRuleInput, MatchedStylesheetRuleInput,
};
#[cfg(test)]
pub(crate) use snapshot::cascade_evaluation_debug_snapshot;
pub use sources::{
    CascadeDeclarationSource, CascadeRuleContext, CascadeRuleMatch, CascadeRuleSource,
    InlineStyleDeclarationRef, InlineStyleRuleRef, StylesheetDeclarationRef, StylesheetRuleRef,
};
pub use winners::{
    CandidateDataMismatch, CascadeResolutionError, CascadeWinner, CascadeWinnerEntry,
    CascadeWinnerSet, RuleInputSequenceViolation,
};

pub(crate) use resolved_style::resolve_cascade_style_owned;
pub(crate) use rules::{ValidatedCascadeRuleInputBuilder, ValidatedCascadeRuleInputs};
#[cfg(test)]
pub(crate) use snapshot::append_cascade_evaluation_debug_snapshot;
pub(crate) use winners::{
    CascadeCandidateObservationIndex, CascadeDeclarationCandidate, CascadeEvaluationFailure,
    CascadeEvaluationObserver, CascadeResolutionBudget, CascadeResolutionWorkspace,
    resolve_cascade_winners, resolve_cascade_winners_from_validated_inputs,
};

#[cfg(test)]
mod tests;
