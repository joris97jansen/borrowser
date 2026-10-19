//! Read-only observations of one accepted production frame. No retained cache.
use super::{PageFrameStatus, Tab, state::DocumentParseStatus};
use crate::rendering::{RetainedPaintArtifactKey, RetainedStyleArtifactKey};
use css::{ComputedStyle, StylePhaseOutput, StyledNode};
use html::{Node, internal::Id};
use layout::RetainedLayoutArtifact;

pub struct FrameElementStyle {
    node_id: Id,
    style: ComputedStyle,
}
impl FrameElementStyle {
    pub fn node_id(&self) -> Id {
        self.node_id
    }
    pub fn style(&self) -> &ComputedStyle {
        &self.style
    }
}

pub(crate) struct FrameDraft {
    pub(crate) style_key: RetainedStyleArtifactKey,
    pub(crate) paint_key: RetainedPaintArtifactKey,
    pub(crate) layout: RetainedLayoutArtifact,
    pub(crate) layout_viewport_width: f32,
    styles: Vec<FrameElementStyle>,
}
impl FrameDraft {
    pub(crate) fn from_frame(
        output: &StylePhaseOutput<'_>,
        style_key: RetainedStyleArtifactKey,
        layout: RetainedLayoutArtifact,
        paint_key: RetainedPaintArtifactKey,
        layout_viewport_width: f32,
    ) -> Self {
        fn collect(node: &StyledNode<'_>, styles: &mut Vec<FrameElementStyle>) {
            if matches!(node.node, Node::Element { .. }) {
                styles.push(FrameElementStyle {
                    node_id: node.node_id,
                    style: node.style,
                });
            }
            for child in &node.children {
                collect(child, styles);
            }
        }
        let mut styles = Vec::new();
        collect(output.root(), &mut styles);
        Self {
            style_key,
            paint_key,
            layout,
            layout_viewport_width,
            styles,
        }
    }
}

/// The document borrow prevents navigation/publication while tooling reads the
/// copied frame-local style and layout artifacts. Tooling must own its selected
/// observations before dropping the originating Tab.
pub struct CompletedPageFrame<'tab> {
    document: &'tab Node,
    draft: FrameDraft,
    output: egui::FullOutput,
}
impl CompletedPageFrame<'_> {
    pub fn document(&self) -> &Node {
        self.document
    }
    pub fn element_styles(&self) -> &[FrameElementStyle] {
        &self.draft.styles
    }
    pub fn layout(&self) -> &RetainedLayoutArtifact {
        &self.draft.layout
    }
    pub fn layout_viewport_width(&self) -> f32 {
        self.draft.layout_viewport_width
    }
    pub fn output(&self) -> &egui::FullOutput {
        &self.output
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameObservationError {
    IncompleteDocument,
    DocumentChanged,
    Style(String),
    FrameNotAccepted,
    Discarded,
    InvalidArtifacts,
}

impl Tab {
    pub fn render_observed_frame<'tab>(
        &'tab mut self,
        ctx: &egui::Context,
        input: egui::RawInput,
    ) -> Result<CompletedPageFrame<'tab>, FrameObservationError> {
        if self.document_load.parse_status != DocumentParseStatus::Complete
            || self.dom_handle.is_none()
            || self.page.dom.is_none()
            || self.page.document_mode.is_none()
            || self.loading
        {
            return Err(FrameObservationError::IncompleteDocument);
        }
        let stamp = (
            self.nav_gen,
            self.dom_handle,
            self.dom_version,
            self.page.document_mode,
        );
        let mut draft = None;
        ctx.begin_pass(input);
        let status = self.ui_content_inner(ctx, Some(&mut draft));
        let output = ctx.end_pass();
        let status = status.map_err(|e| FrameObservationError::Style(format!("{e:?}")))?;
        if stamp
            != (
                self.nav_gen,
                self.dom_handle,
                self.dom_version,
                self.page.document_mode,
            )
            || self.document_load.parse_status != DocumentParseStatus::Complete
        {
            return Err(FrameObservationError::DocumentChanged);
        }
        validate_frame_output(status, !self.pending_render_work.is_empty(), &output)?;
        let draft = draft.ok_or(FrameObservationError::InvalidArtifacts)?;
        if !draft.layout_viewport_width.is_finite() || !self.page.accepts_observed_artifacts(&draft)
        {
            return Err(FrameObservationError::InvalidArtifacts);
        }
        Ok(CompletedPageFrame {
            document: self
                .page
                .dom
                .as_deref()
                .ok_or(FrameObservationError::DocumentChanged)?,
            draft,
            output,
        })
    }
}

// Shared by the entry and focused failure regressions. No draft can be published
// when the production frame requests another execution or egui discards it.
pub(super) fn validate_frame_output(
    status: PageFrameStatus,
    pending_work: bool,
    output: &egui::FullOutput,
) -> Result<(), FrameObservationError> {
    if status != PageFrameStatus::Rendered || pending_work {
        return Err(FrameObservationError::FrameNotAccepted);
    }
    if output.platform_output.requested_discard() {
        return Err(FrameObservationError::Discarded);
    }
    Ok(())
}
