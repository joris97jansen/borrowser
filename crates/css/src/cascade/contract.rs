mod declarations;
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
pub use priority::{
    CascadeDeclarationCandidateKey, CascadeImportance, CascadeOrigin, CascadeOriginBand,
    CascadePriority, CascadeSpecificity, CurrentScopeCascadePriorityBand,
};
pub use properties::{
    CascadeInheritance, CascadePropertyId, CascadePropertyMetadata, CascadePropertyRegistration,
    CascadePropertyRegistry, InitialStyleValue, cascade_property_registry,
};
pub use resolved_style::{
    ResolvedStyle, ResolvedStyleBuildError, ResolvedStyleBuilder, ResolvedStyleEntry,
    ResolvedValueSource, resolve_cascade_style, resolve_cascade_style_from_rule_inputs,
    resolve_initial_style,
};
pub use rules::{CascadeRuleInput, CascadeRuleInputBuildError};
pub use snapshot::cascade_evaluation_debug_snapshot;
pub use sources::{
    CascadeDeclarationSource, CascadeRuleContext, CascadeRuleMatch, CascadeRuleSource,
    InlineStyleDeclarationRef, InlineStyleRuleRef, StylesheetDeclarationRef, StylesheetRuleRef,
};
pub use winners::{
    CascadeDeclarationCandidate, CascadeWinner, CascadeWinnerEntry, CascadeWinnerSet,
    resolve_cascade_winners, resolve_cascade_winners_from_rule_inputs,
    sort_candidates_by_cascade_order,
};

pub(crate) use snapshot::append_cascade_evaluation_debug_snapshot;

#[cfg(test)]
mod tests;
