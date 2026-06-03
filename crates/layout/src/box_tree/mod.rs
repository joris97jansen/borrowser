//! Generated box tree for one layout pass.
//!
//! The box tree is the layout engine's explicit frame-local representation of
//! CSS box generation. It records DOM-backed boxes, anonymous boxes,
//! formatting-context participation, containing-block relationships, list
//! markers, and replaced-element metadata before geometry is computed.

mod builder;
mod debug;
mod display;
mod formatting;
mod ids;
mod model;
mod source;

pub use display::{
    AnonymousBoxKind, BoxGenerationRole, BoxSuppressionReason, DisplayBoxBehavior,
    DisplayBoxGeneration, PrincipalBox,
};
pub use formatting::{
    BlockFormattingParticipation, FlexFormattingParticipation, FormattingContextKind,
    InlineFormattingParticipation,
};
pub use ids::{
    BoxId, ContainingBlockId, FormattingContextId, InlineFormattingContextId,
    PositionedContainingBlockId,
};
pub use model::{BoxNode, BoxTree};
pub use source::BoxSource;

#[cfg(test)]
mod tests;
