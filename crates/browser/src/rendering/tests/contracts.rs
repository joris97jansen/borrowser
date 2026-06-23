use crate::rendering::*;

use super::support::*;

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
    assert_eq!(layout.retained_outputs, &[RenderArtifact::LayoutTree]);
    assert_eq!(layout.rebuilt_outputs, &[]);
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
fn render_artifact_ownership_contracts_pin_retained_vs_rebuilt_lifetimes() {
    let contracts = render_artifact_ownership_contracts();
    assert_eq!(contracts.len(), 12);

    let dom = artifact_contract(contracts, RenderArtifact::Dom);
    assert_eq!(dom.semantic_owner, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        dom.retention_owner,
        Some(RenderingSubsystem::BrowserRuntime)
    );
    assert_eq!(dom.lifetime, RenderArtifactLifetime::RetainedAcrossUpdates);

    let resolved = artifact_contract(contracts, RenderArtifact::ResolvedDocumentStyle);
    assert_eq!(resolved.semantic_owner, RenderingSubsystem::CssEngine);
    assert_eq!(
        resolved.retention_owner,
        Some(RenderingSubsystem::BrowserRuntime)
    );
    assert_eq!(
        resolved.lifetime,
        RenderArtifactLifetime::RetainedAcrossUpdates
    );

    let styled = artifact_contract(contracts, RenderArtifact::StyledTree);
    assert_eq!(styled.semantic_owner, RenderingSubsystem::CssEngine);
    assert_eq!(styled.retention_owner, None);
    assert_eq!(
        styled.lifetime,
        RenderArtifactLifetime::BorrowBackedRebuiltOnDemand
    );

    let layout = artifact_contract(contracts, RenderArtifact::LayoutTree);
    assert_eq!(layout.semantic_owner, RenderingSubsystem::LayoutEngine);
    assert_eq!(
        layout.retention_owner,
        Some(RenderingSubsystem::BrowserRuntime)
    );
    assert_eq!(
        layout.lifetime,
        RenderArtifactLifetime::RetainedAcrossUpdates
    );

    let paint = artifact_contract(contracts, RenderArtifact::PaintCommands);
    assert_eq!(paint.semantic_owner, RenderingSubsystem::PaintEngine);
    assert_eq!(paint.retention_owner, None);
    assert_eq!(paint.lifetime, RenderArtifactLifetime::ImmediateFrameOutput);

    let input = artifact_contract(contracts, RenderArtifact::InputState);
    assert_eq!(input.semantic_owner, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        input.retention_owner,
        Some(RenderingSubsystem::BrowserRuntime)
    );
    assert_eq!(
        input.lifetime,
        RenderArtifactLifetime::RetainedAcrossUpdates
    );
}

#[test]
fn render_artifact_ownership_contracts_cover_each_artifact_once() {
    let contracts = render_artifact_ownership_contracts();
    let expected = [
        RenderArtifact::Dom,
        RenderArtifact::StylesheetSet,
        RenderArtifact::ResolvedDocumentStyle,
        RenderArtifact::ComputedDocumentStyle,
        RenderArtifact::StyledTree,
        RenderArtifact::ViewportMetrics,
        RenderArtifact::TextMeasurement,
        RenderArtifact::ReplacedElementMetadata,
        RenderArtifact::LayoutTree,
        RenderArtifact::ResourceState,
        RenderArtifact::InputState,
        RenderArtifact::PaintCommands,
    ];

    for artifact in expected {
        let count = contracts
            .iter()
            .filter(|contract| contract.artifact == artifact)
            .count();
        assert_eq!(
            count, 1,
            "artifact must have exactly one ownership contract: {artifact:?}"
        );
    }

    assert_eq!(contracts.len(), expected.len());
}

#[test]
fn render_extension_hook_contracts_cover_expected_future_work_once() {
    let contracts = render_extension_hook_contracts();
    let expected = [
        RenderExtensionHook::BoxTreeFormalization,
        RenderExtensionHook::ConstraintSizingAndIntrinsicLayout,
        RenderExtensionHook::PaintPrimitiveAndDisplayListExpansion,
        RenderExtensionHook::IncrementalInvalidationAndDependencyTracking,
        RenderExtensionHook::RetainedLayoutState,
        RenderExtensionHook::RetainedPaintSceneState,
        RenderExtensionHook::RuntimeFrameSchedulingIncrementality,
    ];

    for hook in expected {
        let count = contracts
            .iter()
            .filter(|contract| contract.hook == hook)
            .count();
        assert_eq!(
            count, 1,
            "extension hook must have exactly one contract: {hook:?}"
        );
    }

    assert_eq!(contracts.len(), expected.len());
}

#[test]
fn render_extension_hook_contracts_anchor_deferred_work_to_current_pipeline() {
    let phase_contracts = render_phase_contracts();
    let artifact_contracts = render_artifact_ownership_contracts();
    let invalidation_contracts = render_invalidation_request_contracts();

    for hook in render_extension_hook_contracts() {
        assert!(
            !hook.phases.is_empty(),
            "extension hook must anchor to at least one phase: {:?}",
            hook.hook
        );
        assert!(
            !hook.artifacts.is_empty(),
            "extension hook must anchor to at least one artifact: {:?}",
            hook.hook
        );

        for phase in hook.phases {
            assert!(
                phase_contracts
                    .iter()
                    .any(|contract| contract.phase == *phase),
                "extension hook references unknown phase: {:?} -> {:?}",
                hook.hook,
                phase
            );
        }

        for artifact in hook.artifacts {
            assert!(
                artifact_contracts
                    .iter()
                    .any(|contract| contract.artifact == *artifact),
                "extension hook references unknown artifact: {:?} -> {:?}",
                hook.hook,
                artifact
            );
        }

        for entry_point in hook.invalidation_entry_points {
            assert!(
                invalidation_contracts
                    .iter()
                    .any(|contract| contract.entry_point == *entry_point),
                "extension hook references unknown invalidation entry point: {:?} -> {:?}",
                hook.hook,
                entry_point
            );
        }
    }

    let retained_layout = render_extension_hook_contracts()
        .iter()
        .find(|contract| contract.hook == RenderExtensionHook::RetainedLayoutState)
        .expect("retained layout hook");
    assert_eq!(
        retained_layout.integration_owner,
        RenderingSubsystem::BrowserRuntime
    );
    assert!(
        retained_layout
            .artifacts
            .contains(&RenderArtifact::LayoutTree)
    );

    let retained_paint = render_extension_hook_contracts()
        .iter()
        .find(|contract| contract.hook == RenderExtensionHook::RetainedPaintSceneState)
        .expect("retained paint hook");
    assert_eq!(
        retained_paint.integration_owner,
        RenderingSubsystem::BrowserRuntime
    );
    assert!(
        retained_paint
            .artifacts
            .contains(&RenderArtifact::PaintCommands)
    );

    let invalidation = render_extension_hook_contracts()
        .iter()
        .find(|contract| {
            contract.hook == RenderExtensionHook::IncrementalInvalidationAndDependencyTracking
        })
        .expect("incremental invalidation hook");
    let expected_entry_points = render_invalidation_request_contracts()
        .iter()
        .map(|contract| contract.entry_point)
        .collect::<Vec<_>>();
    assert_eq!(
        invalidation.integration_owner,
        RenderingSubsystem::BrowserRuntime
    );
    assert_eq!(
        invalidation.invalidation_entry_points,
        expected_entry_points.as_slice()
    );
}

#[test]
fn phase_contract_outputs_align_with_artifact_lifetimes() {
    let ownership = render_artifact_ownership_contracts();

    for phase in render_phase_contracts() {
        for artifact in phase.retained_outputs {
            let contract = artifact_contract(ownership, *artifact);
            assert_eq!(
                contract.lifetime,
                RenderArtifactLifetime::RetainedAcrossUpdates,
                "phase retained output must have retained artifact lifetime: {artifact:?}"
            );
            assert!(
                contract.retention_owner.is_some(),
                "retained artifact must have a retention owner: {artifact:?}"
            );
        }

        for artifact in phase.rebuilt_outputs {
            let contract = artifact_contract(ownership, *artifact);
            assert_ne!(
                contract.lifetime,
                RenderArtifactLifetime::RetainedAcrossUpdates,
                "rebuilt phase output must not be retained: {artifact:?}"
            );
            assert_eq!(
                contract.retention_owner, None,
                "rebuilt artifact must not have a retention owner: {artifact:?}"
            );
        }
    }
}
