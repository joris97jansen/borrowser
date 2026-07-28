use crate::html5::tree_builder::{TreeBuilderError, TreeBuilderProcessContext};

/// Parser-algorithm outcome for the original tokenizer self-closing flag.
/// Tree-construction handlers do not own this finalization step; dispatch does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum SelfClosingFlagDisposition {
    Acknowledge,
    LeaveUnacknowledged,
}

impl SelfClosingFlagDisposition {
    pub(in crate::html5::tree_builder) fn apply(
        self,
        context: &mut TreeBuilderProcessContext<'_>,
        self_closing: bool,
    ) -> Result<(), TreeBuilderError> {
        if self_closing && self == Self::Acknowledge {
            context.acknowledge_self_closing_flag()?;
        }
        Ok(())
    }
}
