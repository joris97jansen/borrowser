//! Paint architecture and ordering contracts for the currently supported subset.
//!
//! These tables are contract metadata only. They do not introduce a display
//! list, retained paint scene, compositor model, or new visual behavior.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintContractOwner {
    Layout,
    Paint,
    Gfx,
    BrowserRuntime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintArchitectureArtifact {
    LayoutGeometry,
    FormattingOutput,
    OverflowPolicy,
    OverflowClipMetadata,
    PaintPhaseInput,
    StructuredPaintInput,
    PaintTree,
    PaintPrimitives,
    PaintArgs,
    ImmediatePaintOutput,
    LowLevelDrawExecution,
    PhaseOrchestration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintArchitectureRole {
    OwnsSemanticData,
    ConsumesSemanticInput,
    BuildsSemanticPaintModel,
    DefinesPaintPrimitiveVocabulary,
    ConsumesRuntimeContext,
    EmitsImmediateOutput,
    ExecutesDrawCommands,
    OrchestratesPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintArchitectureContract {
    pub artifact: PaintArchitectureArtifact,
    pub owner: PaintContractOwner,
    pub role: PaintArchitectureRole,
    pub retained: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintOrderStep {
    LayoutBoxPreorderTraversal,
    BoxBackground,
    BoxBorder,
    ListMarker,
    OverflowClipForContents,
    InlineFormattingContent,
    ChildSubtree,
    BoxOutline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintOrderSource {
    LayoutBoxPreorder,
    ComputedStyleOnLayoutBox,
    ComputedBorderOnLayoutBox,
    ComputedOutlineOnLayoutBox,
    LayoutListMarkerMetadata,
    LayoutOverflowClipMetadata,
    LayoutInlineFragments,
    LayoutChildOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintOrderContract {
    pub step: PaintOrderStep,
    pub owner: PaintContractOwner,
    pub source: PaintOrderSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintPrimitiveContractKind {
    Background,
    Border,
    Outline,
    ListMarker,
    OverflowClip,
    Text,
    TextDecoration,
    InlineBox,
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintPrimitiveContractSource {
    LayoutGeometry,
    ComputedStyleOnLayoutBox,
    LayoutListMarkerMetadata,
    LayoutOverflowClipMetadata,
    LayoutInlineFragments,
    LayoutReplacedMetadata,
    ComputedBorderModel,
    ComputedOutlineModel,
    ComputedTextDecorationModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintPrimitiveContract {
    pub primitive: PaintPrimitiveContractKind,
    pub owner: PaintContractOwner,
    pub source: PaintPrimitiveContractSource,
    pub backend_specific: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintExcludedFeature {
    UnsupportedBorderStyles,
    BorderRadius,
    BorderImage,
    UnsupportedOutlineStyles,
    OutlineOffset,
    UnsupportedTextDecorationFeatures,
    FullCssPaintingOrder,
    StackingContexts,
    ZIndex,
    Compositing,
    GpuPipeline,
    RetainedDisplayLists,
    RetainedPaintScenes,
    Scrollbars,
    ScrollOffsets,
    PixelSnapshotTesting,
    NewVisualBehavior,
}

static PAINT_ARCHITECTURE_CONTRACTS: [PaintArchitectureContract; 12] = [
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::LayoutGeometry,
        owner: PaintContractOwner::Layout,
        role: PaintArchitectureRole::OwnsSemanticData,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::FormattingOutput,
        owner: PaintContractOwner::Layout,
        role: PaintArchitectureRole::OwnsSemanticData,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::OverflowPolicy,
        owner: PaintContractOwner::Layout,
        role: PaintArchitectureRole::OwnsSemanticData,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::OverflowClipMetadata,
        owner: PaintContractOwner::Layout,
        role: PaintArchitectureRole::OwnsSemanticData,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::PaintPhaseInput,
        owner: PaintContractOwner::Paint,
        role: PaintArchitectureRole::ConsumesSemanticInput,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::StructuredPaintInput,
        owner: PaintContractOwner::Paint,
        role: PaintArchitectureRole::BuildsSemanticPaintModel,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::PaintTree,
        owner: PaintContractOwner::Paint,
        role: PaintArchitectureRole::BuildsSemanticPaintModel,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::PaintPrimitives,
        owner: PaintContractOwner::Paint,
        role: PaintArchitectureRole::DefinesPaintPrimitiveVocabulary,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::PaintArgs,
        owner: PaintContractOwner::Gfx,
        role: PaintArchitectureRole::ConsumesRuntimeContext,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::ImmediatePaintOutput,
        owner: PaintContractOwner::Paint,
        role: PaintArchitectureRole::EmitsImmediateOutput,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::LowLevelDrawExecution,
        owner: PaintContractOwner::Gfx,
        role: PaintArchitectureRole::ExecutesDrawCommands,
        retained: false,
    },
    PaintArchitectureContract {
        artifact: PaintArchitectureArtifact::PhaseOrchestration,
        owner: PaintContractOwner::BrowserRuntime,
        role: PaintArchitectureRole::OrchestratesPhase,
        retained: false,
    },
];

static PAINT_ORDER_CONTRACTS: [PaintOrderContract; 8] = [
    PaintOrderContract {
        step: PaintOrderStep::LayoutBoxPreorderTraversal,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::LayoutBoxPreorder,
    },
    PaintOrderContract {
        step: PaintOrderStep::BoxBackground,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::ComputedStyleOnLayoutBox,
    },
    PaintOrderContract {
        step: PaintOrderStep::BoxBorder,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::ComputedBorderOnLayoutBox,
    },
    PaintOrderContract {
        step: PaintOrderStep::ListMarker,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::LayoutListMarkerMetadata,
    },
    PaintOrderContract {
        step: PaintOrderStep::OverflowClipForContents,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::LayoutOverflowClipMetadata,
    },
    PaintOrderContract {
        step: PaintOrderStep::InlineFormattingContent,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::LayoutInlineFragments,
    },
    PaintOrderContract {
        step: PaintOrderStep::ChildSubtree,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::LayoutChildOrder,
    },
    PaintOrderContract {
        step: PaintOrderStep::BoxOutline,
        owner: PaintContractOwner::Paint,
        source: PaintOrderSource::ComputedOutlineOnLayoutBox,
    },
];

static PAINT_PRIMITIVE_CONTRACTS: [PaintPrimitiveContract; 9] = [
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::Background,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::ComputedStyleOnLayoutBox,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::Border,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::ComputedBorderModel,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::Outline,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::ComputedOutlineModel,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::ListMarker,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::LayoutListMarkerMetadata,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::OverflowClip,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::LayoutOverflowClipMetadata,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::Text,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::LayoutInlineFragments,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::TextDecoration,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::ComputedTextDecorationModel,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::InlineBox,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::LayoutInlineFragments,
        backend_specific: false,
    },
    PaintPrimitiveContract {
        primitive: PaintPrimitiveContractKind::Replaced,
        owner: PaintContractOwner::Paint,
        source: PaintPrimitiveContractSource::LayoutReplacedMetadata,
        backend_specific: false,
    },
];

static PAINT_EXCLUDED_FEATURES: [PaintExcludedFeature; 17] = [
    PaintExcludedFeature::UnsupportedBorderStyles,
    PaintExcludedFeature::BorderRadius,
    PaintExcludedFeature::BorderImage,
    PaintExcludedFeature::UnsupportedOutlineStyles,
    PaintExcludedFeature::OutlineOffset,
    PaintExcludedFeature::UnsupportedTextDecorationFeatures,
    PaintExcludedFeature::FullCssPaintingOrder,
    PaintExcludedFeature::StackingContexts,
    PaintExcludedFeature::ZIndex,
    PaintExcludedFeature::Compositing,
    PaintExcludedFeature::GpuPipeline,
    PaintExcludedFeature::RetainedDisplayLists,
    PaintExcludedFeature::RetainedPaintScenes,
    PaintExcludedFeature::Scrollbars,
    PaintExcludedFeature::ScrollOffsets,
    PaintExcludedFeature::PixelSnapshotTesting,
    PaintExcludedFeature::NewVisualBehavior,
];

pub fn paint_architecture_contracts() -> &'static [PaintArchitectureContract] {
    &PAINT_ARCHITECTURE_CONTRACTS
}

pub fn paint_order_contracts() -> &'static [PaintOrderContract] {
    &PAINT_ORDER_CONTRACTS
}

pub fn paint_primitive_contracts() -> &'static [PaintPrimitiveContract] {
    &PAINT_PRIMITIVE_CONTRACTS
}

pub fn paint_excluded_features() -> &'static [PaintExcludedFeature] {
    &PAINT_EXCLUDED_FEATURES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn architecture_contract(
        artifact: PaintArchitectureArtifact,
    ) -> &'static PaintArchitectureContract {
        paint_architecture_contracts()
            .iter()
            .find(|contract| contract.artifact == artifact)
            .expect("paint architecture artifact contract")
    }

    #[test]
    fn paint_order_contract_exposes_supported_subset_deterministically() {
        let first = paint_order_contracts();
        let second = paint_order_contracts();
        assert_eq!(first, second);
        assert_eq!(
            first,
            &[
                PaintOrderContract {
                    step: PaintOrderStep::LayoutBoxPreorderTraversal,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::LayoutBoxPreorder,
                },
                PaintOrderContract {
                    step: PaintOrderStep::BoxBackground,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::ComputedStyleOnLayoutBox,
                },
                PaintOrderContract {
                    step: PaintOrderStep::BoxBorder,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::ComputedBorderOnLayoutBox,
                },
                PaintOrderContract {
                    step: PaintOrderStep::ListMarker,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::LayoutListMarkerMetadata,
                },
                PaintOrderContract {
                    step: PaintOrderStep::OverflowClipForContents,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::LayoutOverflowClipMetadata,
                },
                PaintOrderContract {
                    step: PaintOrderStep::InlineFormattingContent,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::LayoutInlineFragments,
                },
                PaintOrderContract {
                    step: PaintOrderStep::ChildSubtree,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::LayoutChildOrder,
                },
                PaintOrderContract {
                    step: PaintOrderStep::BoxOutline,
                    owner: PaintContractOwner::Paint,
                    source: PaintOrderSource::ComputedOutlineOnLayoutBox,
                },
            ]
        );
    }

    #[test]
    fn paint_architecture_contract_keeps_layout_owned_geometry_overflow_and_clips() {
        for artifact in [
            PaintArchitectureArtifact::LayoutGeometry,
            PaintArchitectureArtifact::FormattingOutput,
            PaintArchitectureArtifact::OverflowPolicy,
            PaintArchitectureArtifact::OverflowClipMetadata,
        ] {
            let contract = architecture_contract(artifact);
            assert_eq!(contract.owner, PaintContractOwner::Layout);
            assert_eq!(contract.role, PaintArchitectureRole::OwnsSemanticData);
            assert!(!contract.retained);
        }
    }

    #[test]
    fn paint_architecture_contract_consumes_phase_input_and_emits_immediate_output() {
        let input = architecture_contract(PaintArchitectureArtifact::PaintPhaseInput);
        assert_eq!(input.owner, PaintContractOwner::Paint);
        assert_eq!(input.role, PaintArchitectureRole::ConsumesSemanticInput);
        assert!(!input.retained);

        for artifact in [
            PaintArchitectureArtifact::StructuredPaintInput,
            PaintArchitectureArtifact::PaintTree,
        ] {
            let contract = architecture_contract(artifact);
            assert_eq!(contract.owner, PaintContractOwner::Paint);
            assert_eq!(
                contract.role,
                PaintArchitectureRole::BuildsSemanticPaintModel
            );
            assert!(!contract.retained);
        }

        let primitives = architecture_contract(PaintArchitectureArtifact::PaintPrimitives);
        assert_eq!(primitives.owner, PaintContractOwner::Paint);
        assert_eq!(
            primitives.role,
            PaintArchitectureRole::DefinesPaintPrimitiveVocabulary
        );
        assert!(!primitives.retained);

        let args = architecture_contract(PaintArchitectureArtifact::PaintArgs);
        assert_eq!(args.owner, PaintContractOwner::Gfx);
        assert_eq!(args.role, PaintArchitectureRole::ConsumesRuntimeContext);
        assert!(!args.retained);

        let output = architecture_contract(PaintArchitectureArtifact::ImmediatePaintOutput);
        assert_eq!(output.owner, PaintContractOwner::Paint);
        assert_eq!(output.role, PaintArchitectureRole::EmitsImmediateOutput);
        assert!(!output.retained);
    }

    #[test]
    fn paint_primitive_contracts_are_paint_owned_and_backend_independent() {
        assert_eq!(
            paint_primitive_contracts(),
            &[
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::Background,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::ComputedStyleOnLayoutBox,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::Border,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::ComputedBorderModel,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::Outline,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::ComputedOutlineModel,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::ListMarker,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::LayoutListMarkerMetadata,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::OverflowClip,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::LayoutOverflowClipMetadata,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::Text,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::LayoutInlineFragments,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::TextDecoration,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::ComputedTextDecorationModel,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::InlineBox,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::LayoutInlineFragments,
                    backend_specific: false,
                },
                PaintPrimitiveContract {
                    primitive: PaintPrimitiveContractKind::Replaced,
                    owner: PaintContractOwner::Paint,
                    source: PaintPrimitiveContractSource::LayoutReplacedMetadata,
                    backend_specific: false,
                },
            ]
        );
    }

    #[test]
    fn paint_contract_explicitly_excludes_deferred_visual_and_scene_features() {
        let excluded = paint_excluded_features();
        assert_eq!(
            excluded,
            &[
                PaintExcludedFeature::UnsupportedBorderStyles,
                PaintExcludedFeature::BorderRadius,
                PaintExcludedFeature::BorderImage,
                PaintExcludedFeature::UnsupportedOutlineStyles,
                PaintExcludedFeature::OutlineOffset,
                PaintExcludedFeature::UnsupportedTextDecorationFeatures,
                PaintExcludedFeature::FullCssPaintingOrder,
                PaintExcludedFeature::StackingContexts,
                PaintExcludedFeature::ZIndex,
                PaintExcludedFeature::Compositing,
                PaintExcludedFeature::GpuPipeline,
                PaintExcludedFeature::RetainedDisplayLists,
                PaintExcludedFeature::RetainedPaintScenes,
                PaintExcludedFeature::Scrollbars,
                PaintExcludedFeature::ScrollOffsets,
                PaintExcludedFeature::PixelSnapshotTesting,
                PaintExcludedFeature::NewVisualBehavior,
            ]
        );
    }
}
