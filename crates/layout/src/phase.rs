use css::{StylePhaseOutput, StyledNode};

use crate::{
    LayoutBox, OutOfFlowLayoutParticipant, Rectangle, ReplacedElementInfoProvider, TextMeasurer,
};

/// Structured layout-phase input consumed by the layout engine.
///
/// `'style_tree` is the borrow of the rebuilt style-phase output for this
/// pipeline execution. `'dom` is the lifetime of DOM references stored inside
/// `StyledNode`. Keeping them distinct avoids over-constraining layout to treat
/// a frame-scoped style-tree borrow as if it were the DOM lifetime itself.
pub struct LayoutPhaseInput<'style_tree, 'dom, 'runtime> {
    style_root: &'style_tree StyledNode<'dom>,
    available_width: f32,
    measurer: &'runtime dyn TextMeasurer,
    replaced_info: Option<&'runtime dyn ReplacedElementInfoProvider>,
}

impl<'style_tree, 'dom, 'runtime> LayoutPhaseInput<'style_tree, 'dom, 'runtime> {
    pub fn new(
        style_root: &'style_tree StyledNode<'dom>,
        available_width: f32,
        measurer: &'runtime dyn TextMeasurer,
        replaced_info: Option<&'runtime dyn ReplacedElementInfoProvider>,
    ) -> Self {
        Self {
            style_root,
            available_width,
            measurer,
            replaced_info,
        }
    }

    pub fn from_style_output(
        style_output: &'style_tree StylePhaseOutput<'dom>,
        available_width: f32,
        measurer: &'runtime dyn TextMeasurer,
        replaced_info: Option<&'runtime dyn ReplacedElementInfoProvider>,
    ) -> Self {
        Self::new(
            style_output.root(),
            available_width,
            measurer,
            replaced_info,
        )
    }

    pub fn style_root(&self) -> &'style_tree StyledNode<'dom> {
        self.style_root
    }

    pub fn available_width(&self) -> f32 {
        self.available_width
    }

    pub fn measurer(&self) -> &'runtime dyn TextMeasurer {
        self.measurer
    }

    pub fn replaced_info(&self) -> Option<&'runtime dyn ReplacedElementInfoProvider> {
        self.replaced_info
    }
}

/// Structured layout-phase output handed to downstream paint and input phases.
///
/// `available_width` is stored explicitly as part of the layout environment for
/// this pass. It must not be inferred from `root.rect.width`, because future
/// layout features may allow those values to diverge.
pub struct LayoutPhaseOutput<'style_tree, 'dom> {
    root: LayoutBox<'style_tree, 'dom>,
    available_width: f32,
    out_of_flow_participants: Vec<OutOfFlowLayoutParticipant>,
}

impl<'style_tree, 'dom> LayoutPhaseOutput<'style_tree, 'dom> {
    pub fn new(root: LayoutBox<'style_tree, 'dom>, available_width: f32) -> Self {
        let out_of_flow_participants = collect_out_of_flow_participants(&root);
        Self {
            root,
            available_width,
            out_of_flow_participants,
        }
    }

    pub fn root(&self) -> &LayoutBox<'style_tree, 'dom> {
        &self.root
    }

    pub fn into_root(self) -> LayoutBox<'style_tree, 'dom> {
        self.root
    }

    pub fn document_rect(&self) -> Rectangle {
        self.root.rect
    }

    pub fn viewport_width(&self) -> f32 {
        self.available_width
    }

    pub fn out_of_flow_participants(&self) -> &[OutOfFlowLayoutParticipant] {
        &self.out_of_flow_participants
    }

    pub fn content_height(&self) -> f32 {
        self.root.rect.height
    }
}

fn collect_out_of_flow_participants(root: &LayoutBox<'_, '_>) -> Vec<OutOfFlowLayoutParticipant> {
    let mut participants = Vec::new();
    collect_out_of_flow_participants_from_box(root, &mut participants);
    participants
}

fn collect_out_of_flow_participants_from_box(
    layout: &LayoutBox<'_, '_>,
    participants: &mut Vec<OutOfFlowLayoutParticipant>,
) {
    if let Some(kind) = layout.flow_participation().out_of_flow_kind() {
        let positioned_containing_block = layout.positioned_containing_block().expect(
            "Y5 must resolve positioned containing blocks before Y6 tracks out-of-flow participants",
        );
        participants.push(OutOfFlowLayoutParticipant::new(
            layout.box_id(),
            kind,
            positioned_containing_block,
        ));
    }

    for child in &layout.children {
        collect_out_of_flow_participants_from_box(child, participants);
    }
}
