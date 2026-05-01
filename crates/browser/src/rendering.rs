//! Rendering pipeline contracts and debug surfaces.
//!
//! Milestone V formalizes the rendering pipeline without prematurely adding
//! retained layout or paint caches. This module records the current ownership
//! boundaries, phase I/O, rebuild triggers, and runtime-visible retained vs.
//! rebuilt state so later rendering work can evolve against explicit contracts.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingPhase {
    Style,
    Layout,
    Paint,
    FrameOrchestration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingSubsystem {
    BrowserRuntime,
    BrowserView,
    CssEngine,
    GfxViewport,
    LayoutEngine,
    PaintEngine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderArtifact {
    Dom,
    StylesheetSet,
    ResolvedDocumentStyle,
    ComputedDocumentStyle,
    StyledTree,
    ViewportMetrics,
    TextMeasurement,
    ReplacedElementMetadata,
    LayoutTree,
    ResourceState,
    InputState,
    PaintCommands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderRebuildTrigger {
    DomReplaced,
    DomStructureChanged,
    DomAttributesChanged,
    DomTextChanged,
    StylesheetSetChanged,
    StyleOutputsChanged,
    ViewportChanged,
    ResourceStateChanged,
    InputStateChanged,
    LayoutOutputsChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderingPhaseContract {
    pub phase: RenderingPhase,
    pub coordinator: RenderingSubsystem,
    pub engine_owner: RenderingSubsystem,
    pub consumes: &'static [RenderArtifact],
    pub produces: &'static [RenderArtifact],
    pub retained_outputs: &'static [RenderArtifact],
    pub rebuilt_outputs: &'static [RenderArtifact],
    pub rebuild_triggers: &'static [RenderRebuildTrigger],
}

const FRAME_ORCHESTRATION_CONSUMES: &[RenderArtifact] = &[
    RenderArtifact::StyledTree,
    RenderArtifact::ViewportMetrics,
    RenderArtifact::ResourceState,
    RenderArtifact::InputState,
];
const FRAME_ORCHESTRATION_PRODUCES: &[RenderArtifact] =
    &[RenderArtifact::LayoutTree, RenderArtifact::PaintCommands];
const FRAME_ORCHESTRATION_REBUILDS: &[RenderArtifact] =
    &[RenderArtifact::LayoutTree, RenderArtifact::PaintCommands];
const FRAME_ORCHESTRATION_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::StyleOutputsChanged,
    RenderRebuildTrigger::DomTextChanged,
    RenderRebuildTrigger::ViewportChanged,
    RenderRebuildTrigger::ResourceStateChanged,
    RenderRebuildTrigger::InputStateChanged,
];

const STYLE_CONSUMES: &[RenderArtifact] = &[RenderArtifact::Dom, RenderArtifact::StylesheetSet];
const STYLE_PRODUCES: &[RenderArtifact] = &[
    RenderArtifact::ResolvedDocumentStyle,
    RenderArtifact::ComputedDocumentStyle,
    RenderArtifact::StyledTree,
];
const STYLE_RETAINED: &[RenderArtifact] = &[
    RenderArtifact::ResolvedDocumentStyle,
    RenderArtifact::ComputedDocumentStyle,
];
const STYLE_REBUILDS: &[RenderArtifact] = &[RenderArtifact::StyledTree];
const STYLE_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::DomReplaced,
    RenderRebuildTrigger::DomStructureChanged,
    RenderRebuildTrigger::DomAttributesChanged,
    RenderRebuildTrigger::StylesheetSetChanged,
];

const LAYOUT_CONSUMES: &[RenderArtifact] = &[
    RenderArtifact::StyledTree,
    RenderArtifact::ViewportMetrics,
    RenderArtifact::TextMeasurement,
    RenderArtifact::ReplacedElementMetadata,
];
const LAYOUT_PRODUCES: &[RenderArtifact] = &[RenderArtifact::LayoutTree];
const LAYOUT_REBUILDS: &[RenderArtifact] = &[RenderArtifact::LayoutTree];
const LAYOUT_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::StyleOutputsChanged,
    RenderRebuildTrigger::DomTextChanged,
    RenderRebuildTrigger::ViewportChanged,
    RenderRebuildTrigger::ResourceStateChanged,
];

const PAINT_CONSUMES: &[RenderArtifact] = &[
    RenderArtifact::LayoutTree,
    RenderArtifact::ResourceState,
    RenderArtifact::InputState,
];
const PAINT_PRODUCES: &[RenderArtifact] = &[RenderArtifact::PaintCommands];
const PAINT_REBUILDS: &[RenderArtifact] = &[RenderArtifact::PaintCommands];
const PAINT_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::LayoutOutputsChanged,
    RenderRebuildTrigger::ResourceStateChanged,
    RenderRebuildTrigger::InputStateChanged,
];

static RENDER_PHASE_CONTRACTS: [RenderingPhaseContract; 4] = [
    RenderingPhaseContract {
        phase: RenderingPhase::Style,
        coordinator: RenderingSubsystem::BrowserRuntime,
        engine_owner: RenderingSubsystem::CssEngine,
        consumes: STYLE_CONSUMES,
        produces: STYLE_PRODUCES,
        retained_outputs: STYLE_RETAINED,
        rebuilt_outputs: STYLE_REBUILDS,
        rebuild_triggers: STYLE_TRIGGERS,
    },
    RenderingPhaseContract {
        phase: RenderingPhase::Layout,
        coordinator: RenderingSubsystem::GfxViewport,
        engine_owner: RenderingSubsystem::LayoutEngine,
        consumes: LAYOUT_CONSUMES,
        produces: LAYOUT_PRODUCES,
        retained_outputs: &[],
        rebuilt_outputs: LAYOUT_REBUILDS,
        rebuild_triggers: LAYOUT_TRIGGERS,
    },
    RenderingPhaseContract {
        phase: RenderingPhase::Paint,
        coordinator: RenderingSubsystem::GfxViewport,
        engine_owner: RenderingSubsystem::PaintEngine,
        consumes: PAINT_CONSUMES,
        produces: PAINT_PRODUCES,
        retained_outputs: &[],
        rebuilt_outputs: PAINT_REBUILDS,
        rebuild_triggers: PAINT_TRIGGERS,
    },
    RenderingPhaseContract {
        phase: RenderingPhase::FrameOrchestration,
        coordinator: RenderingSubsystem::BrowserView,
        engine_owner: RenderingSubsystem::GfxViewport,
        consumes: FRAME_ORCHESTRATION_CONSUMES,
        produces: FRAME_ORCHESTRATION_PRODUCES,
        retained_outputs: &[],
        rebuilt_outputs: FRAME_ORCHESTRATION_REBUILDS,
        rebuild_triggers: FRAME_ORCHESTRATION_TRIGGERS,
    },
];

/// Stable rendering phase contract table.
///
/// `FrameOrchestration` is intentionally a runtime coordination phase, not a
/// semantic rendering engine phase like style, layout, or paint.
pub fn render_phase_contracts() -> &'static [RenderingPhaseContract] {
    &RENDER_PHASE_CONTRACTS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderArtifactState {
    Absent,
    RetainedFresh,
    RetainedStale,
    BorrowBackedRebuiltOnDemand,
    /// Rebuilt during frame execution rather than retained in page state.
    FrameLocalRebuiltPerFrame,
    /// Emitted during paint for the current frame rather than retained.
    ImmediateFrameOutput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleInvalidationState {
    None,
    Full,
    AttributeSuffix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderPipelineDebugSnapshot {
    pub has_dom: bool,
    pub resolved_styles: RenderArtifactState,
    pub computed_styles: RenderArtifactState,
    pub styled_tree: RenderArtifactState,
    pub layout_tree: RenderArtifactState,
    pub paint_output: RenderArtifactState,
    pub style_dirty: bool,
    pub layout_dirty: bool,
    pub style_invalidation: StyleInvalidationState,
}

#[cfg(test)]
mod tests {
    use super::{
        RenderArtifact, RenderArtifactState, RenderPipelineDebugSnapshot, RenderRebuildTrigger,
        RenderingPhase, RenderingSubsystem, StyleInvalidationState, render_phase_contracts,
    };
    use crate::page::{PageState, RestyleHint};
    use gfx::paint::PaintPhaseInput;
    use html::{HtmlParseOptions, Node, parse_document};
    use layout::{LayoutBox, LayoutPhaseInput, TextMeasurer, layout_document};
    use std::sync::Arc;

    #[test]
    fn render_phase_contracts_pin_expected_phase_boundaries() {
        let contracts = render_phase_contracts();
        assert_eq!(contracts.len(), 4);

        let orchestration = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::FrameOrchestration)
            .expect("frame orchestration contract");
        assert_eq!(orchestration.coordinator, RenderingSubsystem::BrowserView);
        assert_eq!(orchestration.engine_owner, RenderingSubsystem::GfxViewport);
        assert_eq!(
            orchestration.consumes,
            &[
                RenderArtifact::StyledTree,
                RenderArtifact::ViewportMetrics,
                RenderArtifact::ResourceState,
                RenderArtifact::InputState,
            ]
        );
        assert_eq!(
            orchestration.produces,
            &[RenderArtifact::LayoutTree, RenderArtifact::PaintCommands]
        );

        let style = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::Style)
            .expect("style contract");
        assert_eq!(style.coordinator, RenderingSubsystem::BrowserRuntime);
        assert_eq!(style.engine_owner, RenderingSubsystem::CssEngine);
        assert_eq!(
            style.consumes,
            &[RenderArtifact::Dom, RenderArtifact::StylesheetSet]
        );
        assert_eq!(
            style.produces,
            &[
                RenderArtifact::ResolvedDocumentStyle,
                RenderArtifact::ComputedDocumentStyle,
                RenderArtifact::StyledTree,
            ]
        );
        assert_eq!(
            style.retained_outputs,
            &[
                RenderArtifact::ResolvedDocumentStyle,
                RenderArtifact::ComputedDocumentStyle,
            ]
        );
        assert_eq!(style.rebuilt_outputs, &[RenderArtifact::StyledTree]);

        let layout = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::Layout)
            .expect("layout contract");
        assert_eq!(layout.coordinator, RenderingSubsystem::GfxViewport);
        assert_eq!(layout.engine_owner, RenderingSubsystem::LayoutEngine);
        assert_eq!(
            layout.consumes,
            &[
                RenderArtifact::StyledTree,
                RenderArtifact::ViewportMetrics,
                RenderArtifact::TextMeasurement,
                RenderArtifact::ReplacedElementMetadata,
            ]
        );
        assert_eq!(layout.produces, &[RenderArtifact::LayoutTree]);
        assert_eq!(
            layout.rebuild_triggers,
            &[
                RenderRebuildTrigger::StyleOutputsChanged,
                RenderRebuildTrigger::DomTextChanged,
                RenderRebuildTrigger::ViewportChanged,
                RenderRebuildTrigger::ResourceStateChanged,
            ]
        );

        let paint = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::Paint)
            .expect("paint contract");
        assert_eq!(paint.coordinator, RenderingSubsystem::GfxViewport);
        assert_eq!(paint.engine_owner, RenderingSubsystem::PaintEngine);
        assert_eq!(
            paint.consumes,
            &[
                RenderArtifact::LayoutTree,
                RenderArtifact::ResourceState,
                RenderArtifact::InputState,
            ]
        );
        assert_eq!(paint.produces, &[RenderArtifact::PaintCommands]);
        assert_eq!(
            paint.rebuild_triggers,
            &[
                RenderRebuildTrigger::LayoutOutputsChanged,
                RenderRebuildTrigger::ResourceStateChanged,
                RenderRebuildTrigger::InputStateChanged,
            ]
        );
    }

    #[test]
    fn debug_snapshot_reports_retained_style_artifacts_and_ephemeral_downstream_trees() {
        let mut page = page_with_dom(
            "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        );

        let style_output = style_output_for_test(&mut page);
        assert_eq!(
            styled_element_color(style_output.root(), "p"),
            (255, 0, 0, 255)
        );
        drop(style_output);

        let snapshot = page.render_pipeline_debug_snapshot();
        assert_eq!(
            snapshot,
            RenderPipelineDebugSnapshot {
                has_dom: true,
                resolved_styles: RenderArtifactState::RetainedFresh,
                computed_styles: RenderArtifactState::RetainedFresh,
                styled_tree: RenderArtifactState::BorrowBackedRebuiltOnDemand,
                layout_tree: RenderArtifactState::FrameLocalRebuiltPerFrame,
                paint_output: RenderArtifactState::ImmediateFrameOutput,
                style_dirty: false,
                layout_dirty: true,
                style_invalidation: StyleInvalidationState::None,
            }
        );
    }

    #[test]
    fn style_to_layout_handoff_uses_explicit_phase_output_models() {
        let mut page = page_with_dom(
            "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        );
        let style_output = style_output_for_test(&mut page);
        let paragraph = find_styled_element(style_output.root(), "p").expect("paragraph");
        let measurer = FixedTextMeasurer;

        let layout_input =
            LayoutPhaseInput::from_style_output(&style_output, 320.0, &measurer, None);
        assert!(std::ptr::eq(layout_input.style_root(), style_output.root()));
        assert_eq!(layout_input.available_width(), 320.0);

        let layout_output = layout_document(layout_input);
        let layout_root = layout_output.root();
        let paragraph_box =
            find_layout_box_by_id(layout_root, paragraph.node_id).expect("paragraph layout box");
        assert_eq!(layout_output.document_rect(), layout_root.rect);
        assert_eq!(layout_output.viewport_width(), 320.0);
        assert_eq!(layout_root.node_id(), style_output.root().node_id);
        assert_eq!(paragraph_box.node_id(), paragraph.node_id);
    }

    #[test]
    fn layout_to_paint_handoff_wraps_layout_phase_output_without_reinterpretation() {
        let mut page = page_with_dom(
            "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        );
        let style_output = style_output_for_test(&mut page);
        let measurer = FixedTextMeasurer;
        let layout_output = layout_document(LayoutPhaseInput::from_style_output(
            &style_output,
            480.0,
            &measurer,
            None,
        ));

        let paint_input = PaintPhaseInput::new(&layout_output);
        assert!(std::ptr::eq(paint_input.layout(), &layout_output));
        assert!(std::ptr::eq(
            paint_input.layout_root(),
            layout_output.root()
        ));
        assert_eq!(
            paint_input.layout().document_rect(),
            layout_output.document_rect()
        );
        assert_eq!(
            paint_input.layout_root().node_id(),
            layout_output.root().node_id()
        );
    }

    #[test]
    fn attribute_mutation_keeps_style_cache_but_marks_it_stale_until_restored() {
        let mut page = page_with_dom(
            "<!doctype html><html><head><style>.hot { color: red; } p { color: black; }</style></head><body><p>Hello</p></body></html>",
        );
        let initial = style_output_for_test(&mut page);
        assert_eq!(styled_element_color(initial.root(), "p"), (0, 0, 0, 255));
        drop(initial);

        let p_id = set_first_element_attr(
            page.dom
                .as_deref_mut()
                .expect("page DOM should exist for mutation"),
            "p",
            "class",
            Some("hot".to_string()),
        );
        let hint = RestyleHint::attributes_changed(vec![p_id]);
        page.mark_dom_changed_for_tests(hint);

        let stale = page.render_pipeline_debug_snapshot();
        assert_eq!(stale.resolved_styles, RenderArtifactState::RetainedStale);
        assert_eq!(stale.computed_styles, RenderArtifactState::RetainedStale);
        assert_eq!(
            stale.style_invalidation,
            StyleInvalidationState::AttributeSuffix
        );
        assert!(stale.style_dirty);
        assert!(stale.layout_dirty);

        let restyled = style_output_for_test(&mut page);
        assert_eq!(styled_element_color(restyled.root(), "p"), (255, 0, 0, 255));
        drop(restyled);

        let refreshed = page.render_pipeline_debug_snapshot();
        assert_eq!(
            refreshed.resolved_styles,
            RenderArtifactState::RetainedFresh
        );
        assert_eq!(
            refreshed.computed_styles,
            RenderArtifactState::RetainedFresh
        );
        assert_eq!(refreshed.style_invalidation, StyleInvalidationState::None);
        assert!(!refreshed.style_dirty);
    }

    #[test]
    fn text_mutation_dirties_layout_without_invalidating_computed_style() {
        let mut page = page_with_dom(
            "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        );
        let initial = style_output_for_test(&mut page);
        assert_eq!(styled_element_color(initial.root(), "p"), (255, 0, 0, 255));
        drop(initial);
        page.clear_layout_dirty_for_tests();

        replace_first_text(
            page.dom
                .as_deref_mut()
                .expect("page DOM should exist for mutation"),
            "Hello",
            "Goodbye",
        );
        let hint = RestyleHint::text_mutated();
        page.mark_dom_changed_for_tests(hint);

        let snapshot = page.render_pipeline_debug_snapshot();
        assert_eq!(snapshot.resolved_styles, RenderArtifactState::RetainedFresh);
        assert_eq!(snapshot.computed_styles, RenderArtifactState::RetainedFresh);
        assert_eq!(snapshot.style_invalidation, StyleInvalidationState::None);
        assert!(!snapshot.style_dirty);
        assert!(snapshot.layout_dirty);
    }

    fn page_with_dom(input: &str) -> PageState {
        let output = parse_document(input, HtmlParseOptions::default()).expect("parse should work");
        let mut page = PageState::new();
        page.start_nav("https://example.com/index.html");
        page.replace_dom(Box::new(output.document), RestyleHint::document_replaced());
        page.reconcile_document_stylesheets();
        page
    }

    fn style_output_for_test(page: &mut PageState) -> css::StylePhaseOutput<'_> {
        page.build_style_phase_output()
            .expect("style phase output should build")
            .expect("document should be styled")
    }

    fn styled_element_color(node: &css::StyledNode<'_>, want_name: &str) -> (u8, u8, u8, u8) {
        find_styled_element(node, want_name)
            .map(|node| node.style.color())
            .expect("styled element should exist")
    }

    fn find_styled_element<'a>(
        node: &'a css::StyledNode<'a>,
        want_name: &str,
    ) -> Option<&'a css::StyledNode<'a>> {
        if let Node::Element { name, .. } = node.node
            && name.as_ref() == want_name
        {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_styled_element(child, want_name))
    }

    fn find_layout_box_by_id<'layout, 'dom>(
        layout: &'layout LayoutBox<'layout, 'dom>,
        want: html::internal::Id,
    ) -> Option<&'layout LayoutBox<'layout, 'dom>> {
        if layout.node_id() == want {
            return Some(layout);
        }

        layout
            .children
            .iter()
            .find_map(|child| find_layout_box_by_id(child, want))
    }

    fn set_first_element_attr(
        node: &mut Node,
        want_name: &str,
        attr_name: &str,
        value: Option<String>,
    ) -> html::internal::Id {
        match node {
            Node::Document { children, .. } => children
                .iter_mut()
                .find_map(|child| {
                    set_first_element_attr_optional(child, want_name, attr_name, value.clone())
                })
                .expect("target element should exist"),
            Node::Element {
                id,
                name,
                attributes,
                children,
                ..
            } => {
                if name.as_ref() == want_name {
                    if let Some(existing) = attributes
                        .iter_mut()
                        .find(|(name, _)| name.eq_ignore_ascii_case(attr_name))
                    {
                        existing.1 = value;
                    } else {
                        attributes.push((Arc::from(attr_name), value));
                    }
                    *id
                } else {
                    children
                        .iter_mut()
                        .find_map(|child| {
                            set_first_element_attr_optional(
                                child,
                                want_name,
                                attr_name,
                                value.clone(),
                            )
                        })
                        .expect("target element should exist")
                }
            }
            Node::Text { .. } | Node::Comment { .. } => panic!("target element should exist"),
        }
    }

    fn set_first_element_attr_optional(
        node: &mut Node,
        want_name: &str,
        attr_name: &str,
        value: Option<String>,
    ) -> Option<html::internal::Id> {
        match node {
            Node::Document { children, .. } => children.iter_mut().find_map(|child| {
                set_first_element_attr_optional(child, want_name, attr_name, value.clone())
            }),
            Node::Element {
                id,
                name,
                attributes,
                children,
                ..
            } => {
                if name.as_ref() == want_name {
                    if let Some(existing) = attributes
                        .iter_mut()
                        .find(|(name, _)| name.eq_ignore_ascii_case(attr_name))
                    {
                        existing.1 = value;
                    } else {
                        attributes.push((Arc::from(attr_name), value));
                    }
                    Some(*id)
                } else {
                    children.iter_mut().find_map(|child| {
                        set_first_element_attr_optional(child, want_name, attr_name, value.clone())
                    })
                }
            }
            Node::Text { .. } | Node::Comment { .. } => None,
        }
    }

    fn replace_first_text(node: &mut Node, before: &str, after: &str) -> html::internal::Id {
        replace_first_text_optional(node, before, after).expect("target text should exist")
    }

    fn replace_first_text_optional(
        node: &mut Node,
        before: &str,
        after: &str,
    ) -> Option<html::internal::Id> {
        match node {
            Node::Document { children, .. } | Node::Element { children, .. } => children
                .iter_mut()
                .find_map(|child| replace_first_text_optional(child, before, after)),
            Node::Text { id, text } if text == before => {
                *text = after.to_string();
                Some(*id)
            }
            Node::Text { .. } | Node::Comment { .. } => None,
        }
    }

    struct FixedTextMeasurer;

    impl TextMeasurer for FixedTextMeasurer {
        fn measure(&self, text: &str, _style: &css::ComputedStyle) -> f32 {
            text.chars().count() as f32 * 8.0
        }

        fn line_height(&self, _style: &css::ComputedStyle) -> f32 {
            16.0
        }
    }
}
