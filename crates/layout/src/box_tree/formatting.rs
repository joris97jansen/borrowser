//! Formatting-context and flow participation classification.

use crate::{OverflowPolicy, classify_replaced_kind};
use css::StyledNode;

use super::display::{
    BoxGenerationRole, DisplayBoxBehavior, DisplayBoxGeneration, PrincipalBox,
    display_box_generation, principal_participates_inline,
};

/// Formatting-context kinds modeled by the current layout subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormattingContextKind {
    /// Borrowser's W6 normal-flow block formatting scope.
    Block,
    /// Borrowser's Z2 flex formatting scope.
    Flex,
}

/// How a generated box participates in the current block-flow model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockFormattingParticipation {
    /// The document/root generated box that seeds the initial block context.
    Root,
    /// A block-level normal-flow participant in an ancestor block context.
    BlockLevel,
    /// Inline-level content participating inside a block container's inline flow.
    InlineLevel,
    /// Atomic inline-level participant whose descendants use its own block scope.
    AtomicInline,
    /// Reserved for generated boxes that are not current block-flow participants.
    None,
}

/// How a generated box participates in the current inline-flow model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineFormattingParticipation {
    /// The box is not an inline formatting participant.
    None,
    /// Inline container whose descendants participate in the same context.
    InlineContainer,
    /// Text run contributing inline text fragments.
    TextRun,
    /// Atomic inline-level box represented as one line-building token.
    AtomicInline,
}

/// How a generated box participates in a flex formatting context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexFormattingParticipation {
    /// The box is not a flex item.
    None,
    /// Direct generated in-flow child of a flex container.
    FlexItem,
}

pub(super) fn principal_establishes_containing_block(principal: PrincipalBox) -> bool {
    matches!(
        principal.behavior(),
        DisplayBoxBehavior::DocumentRoot
            | DisplayBoxBehavior::DocumentElement
            | DisplayBoxBehavior::Block
            | DisplayBoxBehavior::FlexContainer
            | DisplayBoxBehavior::InlineBlock
            | DisplayBoxBehavior::ListItem
            | DisplayBoxBehavior::Anonymous
    )
}

pub(super) fn principal_establishes_formatting_context(
    principal: PrincipalBox,
    styled: &StyledNode<'_>,
) -> Option<FormattingContextKind> {
    match principal.behavior() {
        DisplayBoxBehavior::FlexContainer => Some(FormattingContextKind::Flex),
        DisplayBoxBehavior::DocumentRoot
        | DisplayBoxBehavior::DocumentElement
        | DisplayBoxBehavior::InlineBlock => Some(FormattingContextKind::Block),
        DisplayBoxBehavior::Block | DisplayBoxBehavior::ListItem
            if overflow_establishes_formatting_context(styled) =>
        {
            Some(FormattingContextKind::Block)
        }
        DisplayBoxBehavior::Block
        | DisplayBoxBehavior::ListItem
        | DisplayBoxBehavior::Anonymous
        | DisplayBoxBehavior::Inline
        | DisplayBoxBehavior::TextRun
        | DisplayBoxBehavior::ReplacedInline
        | DisplayBoxBehavior::Marker => None,
    }
}

fn overflow_establishes_formatting_context(styled: &StyledNode<'_>) -> bool {
    OverflowPolicy::from_css_overflow(styled.style.overflow())
        .establishes_independent_formatting_context()
}

pub(super) fn principal_block_formatting_participation(
    principal: PrincipalBox,
) -> BlockFormattingParticipation {
    match principal.behavior() {
        DisplayBoxBehavior::DocumentRoot => BlockFormattingParticipation::Root,
        DisplayBoxBehavior::DocumentElement
        | DisplayBoxBehavior::Block
        | DisplayBoxBehavior::FlexContainer
        | DisplayBoxBehavior::ListItem
        | DisplayBoxBehavior::Anonymous => BlockFormattingParticipation::BlockLevel,
        DisplayBoxBehavior::Inline | DisplayBoxBehavior::TextRun => {
            BlockFormattingParticipation::InlineLevel
        }
        DisplayBoxBehavior::InlineBlock | DisplayBoxBehavior::ReplacedInline => {
            BlockFormattingParticipation::AtomicInline
        }
        DisplayBoxBehavior::Marker => BlockFormattingParticipation::None,
    }
}

pub(super) fn principal_establishes_inline_formatting_context(
    principal: PrincipalBox,
    styled: &StyledNode<'_>,
) -> bool {
    match principal.behavior() {
        DisplayBoxBehavior::DocumentElement
        | DisplayBoxBehavior::Block
        | DisplayBoxBehavior::ListItem => {
            styled_children_form_direct_inline_formatting_context(styled, principal.role())
        }
        DisplayBoxBehavior::InlineBlock => {
            styled_children_form_direct_inline_formatting_context(styled, principal.role())
        }
        DisplayBoxBehavior::DocumentRoot
        | DisplayBoxBehavior::FlexContainer
        | DisplayBoxBehavior::Inline
        | DisplayBoxBehavior::TextRun
        | DisplayBoxBehavior::ReplacedInline
        | DisplayBoxBehavior::Anonymous
        | DisplayBoxBehavior::Marker => false,
    }
}

pub(super) fn principal_inline_formatting_participation(
    principal: PrincipalBox,
) -> InlineFormattingParticipation {
    match principal.behavior() {
        DisplayBoxBehavior::Inline => InlineFormattingParticipation::InlineContainer,
        DisplayBoxBehavior::TextRun => InlineFormattingParticipation::TextRun,
        DisplayBoxBehavior::InlineBlock | DisplayBoxBehavior::ReplacedInline => {
            InlineFormattingParticipation::AtomicInline
        }
        DisplayBoxBehavior::DocumentRoot
        | DisplayBoxBehavior::DocumentElement
        | DisplayBoxBehavior::Block
        | DisplayBoxBehavior::FlexContainer
        | DisplayBoxBehavior::ListItem
        | DisplayBoxBehavior::Anonymous
        | DisplayBoxBehavior::Marker => InlineFormattingParticipation::None,
    }
}

pub(super) fn principal_participates_in_inline_formatting_context(principal: PrincipalBox) -> bool {
    !matches!(
        principal_inline_formatting_participation(principal),
        InlineFormattingParticipation::None
    )
}

fn styled_children_form_direct_inline_formatting_context(
    styled: &StyledNode<'_>,
    parent_role: BoxGenerationRole,
) -> bool {
    let mut has_inline_level = false;
    let mut has_block_level = false;

    for child in &styled.children {
        let Some(child_is_inline_level) = child_participates_inline(child, parent_role) else {
            continue;
        };

        if child_is_inline_level {
            has_inline_level = true;
        } else {
            has_block_level = true;
        }

        if has_inline_level && has_block_level {
            return false;
        }
    }

    has_inline_level
}

fn child_participates_inline(
    child: &StyledNode<'_>,
    parent_role: BoxGenerationRole,
) -> Option<bool> {
    let replaced_kind = classify_replaced_kind(child.node);
    let generation = display_box_generation(child, Some(parent_role), replaced_kind);
    let principal = match generation {
        DisplayBoxGeneration::SuppressSubtree(_) => return None,
        DisplayBoxGeneration::GeneratePrincipalBox(principal) => principal,
    };

    Some(principal_participates_inline(principal))
}
