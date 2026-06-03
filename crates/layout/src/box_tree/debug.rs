//! Stable debug snapshot formatting for generated box trees.

use crate::{
    box_kind_debug_label, intrinsic_size_debug_label, list_marker_debug_label, node_debug_label,
    replaced_kind_debug_label,
};
use css::Display;
use html::internal::Id;
use std::fmt::Write;

use super::display::{AnonymousBoxKind, BoxGenerationRole, DisplayBoxBehavior};
use super::formatting::{
    BlockFormattingParticipation, FlexFormattingParticipation, FormattingContextKind,
    InlineFormattingParticipation,
};
use super::ids::{
    BoxId, ContainingBlockId, FormattingContextId, InlineFormattingContextId,
    PositionedContainingBlockId,
};
use super::model::BoxTree;
use crate::{FlowParticipation, PositioningScheme};

impl<'style_tree, 'dom> BoxTree<'style_tree, 'dom> {
    /// Stable debug snapshot for generated box-tree structure.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "box-tree").expect("write snapshot");
        writeln!(&mut out, "root: {}", box_id_debug_label(self.root)).expect("write snapshot");
        writeln!(&mut out, "boxes: {}", self.len()).expect("write snapshot");
        append_box_node_snapshot(&mut out, self, self.root, 0);
        out
    }
}

fn append_box_node_snapshot(out: &mut String, tree: &BoxTree<'_, '_>, id: BoxId, depth: usize) {
    let node = tree.node(id);
    let indent = "  ".repeat(depth);
    writeln!(
        out,
        "{indent}{}: parent={} cb={} establishes-cb={} position={} flow={} positioned-cb={} establishes-positioned-cb={} fc={} establishes-fc={} block-participation={} flex-participation={} ifc={} establishes-ifc={} inline-participation={} source-id={} source={} role={} kind={} display={} behavior={} children={} marker={} replaced={} intrinsic={}",
        box_id_debug_label(node.id),
        optional_box_id_debug_label(node.parent),
        optional_containing_block_id_debug_label(node.containing_block),
        bool_debug_label(node.establishes_containing_block),
        positioning_scheme_debug_label(node.positioning_scheme),
        flow_participation_debug_label(node.flow_participation),
        optional_positioned_containing_block_id_debug_label(node.positioned_containing_block),
        bool_debug_label(node.establishes_positioned_containing_block),
        optional_formatting_context_id_debug_label(node.formatting_context),
        optional_formatting_context_kind_debug_label(node.establishes_formatting_context),
        block_formatting_participation_debug_label(node.block_formatting_participation),
        flex_formatting_participation_debug_label(node.flex_formatting_participation),
        optional_inline_formatting_context_id_debug_label(node.inline_formatting_context),
        bool_debug_label(node.establishes_inline_formatting_context),
        inline_formatting_participation_debug_label(node.inline_formatting_participation),
        optional_node_id_debug_label(node.direct_node_id()),
        node_debug_label(node.source.anchor_styled_node().node),
        role_debug_label(node.role),
        box_kind_debug_label(node.kind),
        display_debug_label(node.display),
        display_behavior_debug_label(node.display_behavior),
        children_debug_label(&node.children),
        list_marker_debug_label(node.list_marker),
        replaced_kind_debug_label(node.replaced),
        intrinsic_size_debug_label(node.replaced_intrinsic),
    )
    .expect("write snapshot");

    for child in &node.children {
        append_box_node_snapshot(out, tree, *child, depth + 1);
    }
}

fn box_id_debug_label(id: BoxId) -> String {
    format!("b{}", id.index())
}

fn optional_box_id_debug_label(id: Option<BoxId>) -> String {
    id.map(box_id_debug_label)
        .unwrap_or_else(|| "none".to_string())
}

fn optional_containing_block_id_debug_label(id: Option<ContainingBlockId>) -> String {
    id.map(|id| box_id_debug_label(id.box_id()))
        .unwrap_or_else(|| "none".to_string())
}

fn optional_positioned_containing_block_id_debug_label(
    id: Option<PositionedContainingBlockId>,
) -> String {
    id.map(|id| box_id_debug_label(id.box_id()))
        .unwrap_or_else(|| "none".to_string())
}

fn positioning_scheme_debug_label(scheme: PositioningScheme) -> &'static str {
    scheme.as_debug_label()
}

fn flow_participation_debug_label(participation: FlowParticipation) -> &'static str {
    participation.as_debug_label()
}

fn optional_formatting_context_id_debug_label(id: Option<FormattingContextId>) -> String {
    id.map(|id| box_id_debug_label(id.box_id()))
        .unwrap_or_else(|| "none".to_string())
}

fn optional_formatting_context_kind_debug_label(
    kind: Option<FormattingContextKind>,
) -> &'static str {
    match kind {
        Some(FormattingContextKind::Block) => "block",
        Some(FormattingContextKind::Flex) => "flex",
        None => "none",
    }
}

fn block_formatting_participation_debug_label(
    participation: BlockFormattingParticipation,
) -> &'static str {
    match participation {
        BlockFormattingParticipation::Root => "root",
        BlockFormattingParticipation::BlockLevel => "block-level",
        BlockFormattingParticipation::InlineLevel => "inline-level",
        BlockFormattingParticipation::AtomicInline => "atomic-inline",
        BlockFormattingParticipation::None => "none",
    }
}

fn flex_formatting_participation_debug_label(
    participation: FlexFormattingParticipation,
) -> &'static str {
    match participation {
        FlexFormattingParticipation::None => "none",
        FlexFormattingParticipation::FlexItem => "flex-item",
    }
}

fn optional_inline_formatting_context_id_debug_label(
    id: Option<InlineFormattingContextId>,
) -> String {
    id.map(|id| box_id_debug_label(id.box_id()))
        .unwrap_or_else(|| "none".to_string())
}

fn inline_formatting_participation_debug_label(
    participation: InlineFormattingParticipation,
) -> &'static str {
    match participation {
        InlineFormattingParticipation::None => "none",
        InlineFormattingParticipation::InlineContainer => "inline-container",
        InlineFormattingParticipation::TextRun => "text-run",
        InlineFormattingParticipation::AtomicInline => "atomic-inline",
    }
}

fn optional_node_id_debug_label(id: Option<Id>) -> String {
    id.map(|id| id.0.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn bool_debug_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn children_debug_label(children: &[BoxId]) -> String {
    let mut out = String::from("[");
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&box_id_debug_label(*child));
    }
    out.push(']');
    out
}

fn role_debug_label(role: BoxGenerationRole) -> &'static str {
    match role {
        BoxGenerationRole::DocumentRoot => "document-root",
        BoxGenerationRole::DocumentElement => "document-element",
        BoxGenerationRole::OrdinaryElement => "ordinary-element",
        BoxGenerationRole::TextRun => "text-run",
        BoxGenerationRole::Anonymous(AnonymousBoxKind::Block) => "anonymous-block",
        BoxGenerationRole::Anonymous(AnonymousBoxKind::Inline) => "anonymous-inline",
        BoxGenerationRole::Marker => "marker",
    }
}

fn display_debug_label(display: Display) -> &'static str {
    match display {
        Display::Block => "block",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
        Display::ListItem => "list-item",
        Display::Flex => "flex",
        Display::None => "none",
    }
}

fn display_behavior_debug_label(behavior: DisplayBoxBehavior) -> &'static str {
    match behavior {
        DisplayBoxBehavior::DocumentRoot => "document-root",
        DisplayBoxBehavior::DocumentElement => "document-element",
        DisplayBoxBehavior::Block => "block",
        DisplayBoxBehavior::Inline => "inline",
        DisplayBoxBehavior::InlineBlock => "inline-block",
        DisplayBoxBehavior::ListItem => "list-item",
        DisplayBoxBehavior::FlexContainer => "flex-container",
        DisplayBoxBehavior::TextRun => "text-run",
        DisplayBoxBehavior::ReplacedInline => "replaced-inline",
        DisplayBoxBehavior::Anonymous => "anonymous",
        DisplayBoxBehavior::Marker => "marker",
    }
}
