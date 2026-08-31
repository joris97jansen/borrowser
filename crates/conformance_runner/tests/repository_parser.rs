#![cfg(feature = "html-parser")]

use std::path::Path;

use conformance_runner::{
    CapabilityAvailability, ClassificationCompleteness, DerivedPolicyResult, ExecutionAttempt,
    NotAttemptedReason, ObservedExecutionOutcome, ParserObservationProfile,
    ParserObservationSurface, build_report, run_repository_parser_cases,
};
use conformance_test_support::{EngineCapabilityKind, RequirementTag};

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
}

#[test]
fn repository_parser_profiles_execute_and_normalize_orthogonally() {
    let summary = run_repository_parser_cases(repository_root()).expect("AG4 parser run");
    assert_eq!(summary.cases().len(), 7);
    assert!(
        summary
            .cases()
            .windows(2)
            .all(|pair| pair[0].ag.test_id < pair[1].ag.test_id)
    );

    for case in summary.cases().iter().filter(|case| {
        case.ag.test_id.as_str() != "html-tree-construction-repeated-body-unavailable"
    }) {
        assert_eq!(
            case.execution,
            ExecutionAttempt::Attempted {
                outcome: ObservedExecutionOutcome::SemanticPass,
            },
            "{}",
            case.ag.test_id
        );
        assert_eq!(
            case.policy,
            DerivedPolicyResult::ExpectedPass,
            "{}",
            case.ag.test_id
        );
        assert_eq!(
            case.ag.classification,
            ClassificationCompleteness::Classified
        );
        assert_eq!(case.ag.capability, Some(CapabilityAvailability::Available));
        assert_eq!(
            case.ag.requirements,
            [
                RequirementTag::NoJs,
                RequirementTag::RequiresHtmlParserFeature
            ]
        );
    }

    let unavailable = summary
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "html-tree-construction-repeated-body-unavailable")
        .unwrap();
    assert_eq!(
        unavailable.execution,
        ExecutionAttempt::NotAttempted {
            reason: NotAttemptedReason::Eligibility,
            pre_attempt: None,
        }
    );
    assert_eq!(unavailable.execution.observed_outcome(), None);
    assert_eq!(unavailable.policy, DerivedPolicyResult::NotRun);
    let Some(CapabilityAvailability::Unavailable { missing }) = &unavailable.ag.capability else {
        panic!("unavailable metadata remains orthogonal")
    };
    assert_eq!(
        missing
            .iter()
            .map(|item| item.feature.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["merge-attributes-into-existing-body-element"]
    );
    assert!(
        missing
            .iter()
            .all(|item| item.kind == EngineCapabilityKind::HtmlParserFeature)
    );
}

#[test]
fn observation_profiles_have_distinct_reportable_surfaces() {
    let summary = run_repository_parser_cases(repository_root()).unwrap();
    let surfaces = |id: &str| {
        summary
            .cases()
            .iter()
            .find(|case| case.ag.test_id.as_str() == id)
            .unwrap()
            .observations
            .iter()
            .map(|artifact| artifact.surface)
            .collect::<Vec<_>>()
    };
    let tokenizer = surfaces("html-tokenizer-basic-document");
    assert!(tokenizer.contains(&ParserObservationSurface::Tokens));
    assert!(tokenizer.contains(&ParserObservationSurface::ParseErrors));
    assert!(tokenizer.contains(&ParserObservationSurface::UnsupportedFeatures));
    assert!(!tokenizer.contains(&ParserObservationSurface::DocumentMode));
    assert!(!tokenizer.contains(&ParserObservationSurface::Tree));
    assert!(!tokenizer.contains(&ParserObservationSurface::Patches));

    let tree = surfaces("html-tree-construction-basic-document");
    for required in [
        ParserObservationSurface::ParseErrors,
        ParserObservationSurface::DocumentMode,
        ParserObservationSurface::Tree,
        ParserObservationSurface::Patches,
        ParserObservationSurface::Transitions,
    ] {
        assert!(tree.contains(&required), "tree profile misses {required:?}");
    }

    let dom = surfaces("dom-tree-basic-document");
    for reportable in [
        ParserObservationSurface::ParseErrors,
        ParserObservationSurface::ImplementationDiagnostics,
        ParserObservationSurface::DocumentMode,
        ParserObservationSurface::Tree,
        ParserObservationSurface::UnsupportedFeatures,
        ParserObservationSurface::FinalInvariants,
    ] {
        assert!(
            dom.contains(&reportable),
            "DOM profile misses {reportable:?}"
        );
    }
    assert!(!dom.contains(&ParserObservationSurface::Patches));
    assert!(!dom.contains(&ParserObservationSurface::Transitions));
    assert!(summary.cases().iter().any(|case| {
        case.profile == ParserObservationProfile::DomTree
            && case.observations.iter().any(|artifact| {
                artifact.surface == ParserObservationSurface::Tree
                    && artifact.format == "html5-dom-v3"
            })
    }));
}

#[test]
fn repository_report_is_repeatable_path_independent_and_bounded() {
    let first = run_repository_parser_cases(repository_root()).unwrap();
    let second = run_repository_parser_cases(repository_root()).unwrap();
    let first = build_report(first.cases()).unwrap();
    let second = build_report(second.cases()).unwrap();
    assert_eq!(first, second);
    assert!(first.len() < conformance_runner::DEFAULT_REPORT_LIMITS.total_bytes);
    let report = std::str::from_utf8(&first).unwrap();
    assert!(!report.contains(repository_root().to_string_lossy().as_ref()));
    assert!(!report.contains('\r'));
}
