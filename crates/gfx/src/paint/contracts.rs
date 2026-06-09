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
    PaintArgs,
    ImmediatePaintOutput,
    LowLevelDrawExecution,
    PhaseOrchestration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintArchitectureRole {
    OwnsSemanticData,
    ConsumesSemanticInput,
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
    ListMarker,
    OverflowClipForContents,
    InlineFormattingContent,
    ChildSubtree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintOrderSource {
    LayoutBoxPreorder,
    ComputedStyleOnLayoutBox,
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
pub enum PaintExcludedFeature {
    Borders,
    Outlines,
    TextDecorations,
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

static PAINT_ARCHITECTURE_CONTRACTS: [PaintArchitectureContract; 9] = [
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

static PAINT_ORDER_CONTRACTS: [PaintOrderContract; 6] = [
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
];

static PAINT_EXCLUDED_FEATURES: [PaintExcludedFeature; 14] = [
    PaintExcludedFeature::Borders,
    PaintExcludedFeature::Outlines,
    PaintExcludedFeature::TextDecorations,
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
    fn paint_contract_explicitly_excludes_deferred_visual_and_scene_features() {
        let excluded = paint_excluded_features();
        assert_eq!(
            excluded,
            &[
                PaintExcludedFeature::Borders,
                PaintExcludedFeature::Outlines,
                PaintExcludedFeature::TextDecorations,
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
