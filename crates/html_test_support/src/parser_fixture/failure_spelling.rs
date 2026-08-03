use super::model::{
    ExecutionFailureClass, ParserObservationFailureClass, ValidatedFixtureInvariantCode,
};
use html::conformance::{
    ObservationReservationSite, ParserFatalIdentity, ParserObservationExecutionIdentity as I,
    ParserObservationInvariantError as O, ParserReservationSiteIdentity as P,
    ParserTokenizerInvariantError as T, UnsupportedFeatureObservationInvariantError as U,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ParserObservationFailureSpelling {
    pub(super) identity: &'static str,
    pub(super) code: Option<&'static str>,
    pub(super) site: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FailureSpellingError {
    ContradictoryIdentityFields,
    UnknownParserObservationIdentity,
    UnknownTokenizerInvariant,
    UnknownUnsupportedFeatureObservationInvariant,
    UnknownObservationInvariant,
    UnknownParserReservationSite,
    UnknownObservationReservationSite,
    UnknownRunnerInvariant,
}

impl std::fmt::Display for FailureSpellingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ContradictoryIdentityFields => {
                "parser-observation identity fields are incomplete or contradictory"
            }
            Self::UnknownParserObservationIdentity => "unknown parser-observation identity",
            Self::UnknownTokenizerInvariant => "unknown tokenizer invariant code",
            Self::UnknownUnsupportedFeatureObservationInvariant => {
                "unknown unsupported-feature observation invariant code"
            }
            Self::UnknownObservationInvariant => "unknown parser observation invariant code",
            Self::UnknownParserReservationSite => "unknown parser-fatal reservation site",
            Self::UnknownObservationReservationSite => "unknown observation reservation site",
            Self::UnknownRunnerInvariant => "unknown validated-runner invariant code",
        })
    }
}

macro_rules! closed_codec {
    ($format:ident, $parse:ident, $all:ident, $ty:ty, { $($variant:path => $name:literal),+ $(,)? }) => {
        pub(super) const fn $format(value: $ty) -> &'static str {
            match value {
                $($variant => $name),+
            }
        }

        fn $parse(value: &str) -> Option<$ty> {
            match value {
                $($name => Some($variant),)+
                _ => None,
            }
        }

        #[cfg(test)]
        pub(super) const fn $all() -> &'static [$ty] {
            &[$($variant),+]
        }
    };
}

closed_codec!(
    tokenizer_invariant_name,
    parse_tokenizer_invariant,
    all_tokenizer_invariants,
    T,
    {
        T::SelfClosingFlagMissingSolidusPosition => "self-closing-flag-missing-solidus-position",
        T::SolidusPositionWithoutPendingTag => "solidus-position-without-pending-tag",
        T::SolidusPositionOutsideCurrentPendingTag => "solidus-position-outside-current-pending-tag",
        T::SolidusPositionDoesNotReferenceConsumedSlash => "solidus-position-does-not-reference-consumed-slash",
        T::DoctypeNameStartMissingForNameState => "doctype-name-start-missing-for-name-state",
        T::DoctypeNameStartMissingForTailScan => "doctype-name-start-missing-for-tail-scan",
        T::DoctypeNameStartMissingForResourceObservation => "doctype-name-start-missing-for-resource-observation",
        T::DoctypeNameStartAfterCursor => "doctype-name-start-after-cursor",
        T::DoctypeNameRangeInvalid => "doctype-name-range-invalid",
        T::DoctypeTailRangeInvalid => "doctype-tail-range-invalid",
        T::AsciiPrefixCandidateRangeInvalid => "ascii-prefix-candidate-range-invalid",
        T::CommentStateMissingPendingStart => "comment-state-missing-pending-start",
        T::CommentPendingRangeInvalid => "comment-pending-range-invalid",
        T::CommentPendingDelimiterOutsideCurrentRange => "comment-pending-delimiter-outside-current-range",
        T::CommentPendingDelimiterDoesNotMatchState => "comment-pending-delimiter-does-not-match-state",
        T::TextModeEndTagCandidateRangeInvalid => "text-mode-end-tag-candidate-range-invalid",
        T::TextModeEndTagAttributePositionInvalid => "text-mode-end-tag-attribute-position-invalid",
        T::TextModeEndTagSolidusPositionInvalid => "text-mode-end-tag-solidus-position-invalid",
        T::PendingTextRangeInvalid => "pending-text-range-invalid",
        T::CdataStateMissingPendingTextStart => "cdata-state-missing-pending-text-start",
        T::CdataEndDelimiterOutsidePendingTextRange => "cdata-end-delimiter-outside-pending-text-range",
        T::CdataEndDelimiterDoesNotMatchState => "cdata-end-delimiter-does-not-match-state",
        T::ProcessingInstructionStateMissingPendingMetadata => "processing-instruction-state-missing-pending-metadata",
        T::ProcessingInstructionMetadataOutsideState => "processing-instruction-metadata-outside-state",
        T::ProcessingInstructionTargetRangeInvalid => "processing-instruction-target-range-invalid",
        T::ProcessingInstructionDataRangeInvalid => "processing-instruction-data-range-invalid",
        T::ProcessingInstructionTargetStartAfterCursor => "processing-instruction-target-start-after-cursor",
        T::ProcessingInstructionDataStartAfterCursor => "processing-instruction-data-start-after-cursor",
    }
);

closed_codec!(
    unsupported_observation_invariant_name,
    parse_unsupported_observation_invariant,
    all_unsupported_observation_invariants,
    U,
    {
        U::TokenAttributeNameUnavailable => "token-attribute-name-unavailable",
        U::ExistingHtmlElementSemanticsUnavailable => "existing-html-element-semantics-unavailable",
        U::ExistingBodyElementSemanticsUnavailable => "existing-body-element-semantics-unavailable",
        U::ExistingElementIdentityContradiction => "existing-element-identity-contradiction",
    }
);

closed_codec!(
    observation_invariant_name,
    parse_observation_invariant,
    all_observation_invariants,
    O,
    {
        O::ParseErrorOccurrenceOverflow => "parse-error-occurrence-overflow",
        O::ImplementationDiagnosticOccurrenceOverflow => "implementation-diagnostic-occurrence-overflow",
        O::TreeTransitionOccurrenceOverflow => "tree-transition-occurrence-overflow",
        O::UnsupportedFeatureOccurrenceOverflow => "unsupported-feature-occurrence-overflow",
        O::TokenDroppedCountOverflow => "token-dropped-count-overflow",
        O::ParseErrorDroppedCountOverflow => "parse-error-dropped-count-overflow",
        O::ImplementationDiagnosticDroppedCountOverflow => "implementation-diagnostic-dropped-count-overflow",
        O::TreeTransitionDroppedCountOverflow => "tree-transition-dropped-count-overflow",
        O::UnsupportedFeatureDroppedCountOverflow => "unsupported-feature-dropped-count-overflow",
        O::NormalizedPositionOverflow => "normalized-position-overflow",
        O::NormalizedPositionIndexDiscontinuity => "normalized-position-index-discontinuity",
        O::NormalizedPositionIndexMissing => "normalized-position-index-missing",
        O::InvalidNormalizedPositionOffset => "invalid-normalized-position-offset",
        O::PatchDroppedCountOverflow => "patch-dropped-count-overflow",
        O::CanonicalTreeUnitCountOverflow => "canonical-tree-unit-count-overflow",
        O::CanonicalTreeRootNotDocument => "canonical-tree-root-not-document",
        O::UnexpectedLegacyDocumentDoctypeMetadata => "unexpected-legacy-document-doctype-metadata",
        O::MissingHtmlTemplateContents => "missing-html-template-contents",
        O::InvalidTemplateContentsKind => "invalid-template-contents-kind",
        O::CanonicalTreeTraversalContradiction => "canonical-tree-traversal-contradiction",
        O::CanonicalTreePreflightProjectionMismatch => "canonical-tree-preflight-projection-mismatch",
        O::InvalidPatchKey => "invalid-patch-key",
        O::DuplicatePatchCreation => "duplicate-patch-creation",
        O::MissingPatchCreationHistory => "missing-patch-creation-history",
        O::SnapshotLabelSequenceOverflow => "snapshot-label-sequence-overflow",
    }
);

closed_codec!(
    parser_reservation_site_name,
    parse_parser_reservation_site,
    all_parser_reservation_sites,
    P,
    {
        P::KnownTagAtomStorage => "known-tag-atom-storage",
        P::KnownTagLookupStorage => "known-tag-lookup-storage",
        P::TemplateChildStorage => "template-child-storage",
        P::PatchHistoryObservationStorage => "patch-history-observation-storage",
    }
);

closed_codec!(
    observation_reservation_site_name,
    parse_observation_reservation_site,
    all_observation_reservation_sites,
    ObservationReservationSite,
    {
        ObservationReservationSite::CanonicalTreeProjection => "canonical-tree-projection",
        ObservationReservationSite::CanonicalPatchProjection => "canonical-patch-projection",
        ObservationReservationSite::SnapshotLabelStorage => "snapshot-label-storage",
    }
);

closed_codec!(
    runner_invariant_name,
    parse_runner_invariant_inner,
    all_runner_invariants,
    ValidatedFixtureInvariantCode,
    {
        ValidatedFixtureInvariantCode::PlannedReferenceDeliveryMissing => "planned-reference-delivery-missing",
        ValidatedFixtureInvariantCode::PlannedDeliveryMissing => "planned-delivery-missing",
        ValidatedFixtureInvariantCode::DuplicatePlannedDelivery => "duplicate-planned-delivery",
        ValidatedFixtureInvariantCode::RequestedSurfaceUnexpectedlyNotRequested => "requested-surface-unexpectedly-not-requested",
        ValidatedFixtureInvariantCode::RequestedSurfaceUnexpectedlyNotApplicable => "requested-surface-unexpectedly-not-applicable",
        ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyCaptured => "unrequested-surface-unexpectedly-captured",
        ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyIncomplete => "unrequested-surface-unexpectedly-incomplete",
        ValidatedFixtureInvariantCode::SnapshotVariantSurfaceContradiction => "snapshot-variant-surface-contradiction",
        ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction => "canonical-serializer-surface-contradiction",
        ValidatedFixtureInvariantCode::ComparisonSurfaceContradiction => "comparison-surface-contradiction",
        ValidatedFixtureInvariantCode::MissingExecutedDeliveryResult => "missing-executed-delivery-result",
        ValidatedFixtureInvariantCode::DuplicateExecutedDeliveryResult => "duplicate-executed-delivery-result",
        ValidatedFixtureInvariantCode::DuplicateExpectationIdentity => "duplicate-expectation-identity",
    }
);

pub(super) fn parse_parser_observation_failure(
    identity: &str,
    code: Option<&str>,
    site: Option<&str>,
) -> Result<ParserObservationFailureClass, FailureSpellingError> {
    let value = match (identity, code, site) {
        ("parser-fatal-engine-invariant", None, None) => {
            I::ParserFatal(ParserFatalIdentity::EngineInvariant)
        }
        ("parser-fatal-resource-exhaustion", None, Some(site)) => {
            I::ParserFatal(ParserFatalIdentity::ResourceExhaustion(
                parse_parser_reservation_site(site)
                    .ok_or(FailureSpellingError::UnknownParserReservationSite)?,
            ))
        }
        ("parser-invariant", None, None) => I::ParserInvariant,
        ("tokenizer-invariant", Some(code), None) => I::TokenizerInvariant(
            parse_tokenizer_invariant(code)
                .ok_or(FailureSpellingError::UnknownTokenizerInvariant)?,
        ),
        ("token-canonicalization-invariant", None, None) => I::TokenCanonicalizationInvariant,
        ("tree-transition-token-canonicalization-invariant", None, None) => {
            I::TreeTransitionTokenCanonicalizationInvariant
        }
        ("unsupported-feature-observation-invariant", Some(code), None) => {
            I::UnsupportedFeatureObservationInvariant(
                parse_unsupported_observation_invariant(code)
                    .ok_or(FailureSpellingError::UnknownUnsupportedFeatureObservationInvariant)?,
            )
        }
        ("observation-recorder-missing", None, None) => I::ObservationRecorderMissing,
        ("patch-history-capture-missing", None, None) => I::PatchHistoryCaptureMissing,
        ("observation-invariant", Some(code), None) => I::ObservationInvariant(
            parse_observation_invariant(code)
                .ok_or(FailureSpellingError::UnknownObservationInvariant)?,
        ),
        ("observation-resource-exhaustion", None, Some(site)) => I::ResourceExhaustion(
            parse_observation_reservation_site(site)
                .ok_or(FailureSpellingError::UnknownObservationReservationSite)?,
        ),
        (identity, _, _) if !is_parser_observation_identity(identity) => {
            return Err(FailureSpellingError::UnknownParserObservationIdentity);
        }
        _ => return Err(FailureSpellingError::ContradictoryIdentityFields),
    };
    Ok(value)
}

fn is_parser_observation_identity(identity: &str) -> bool {
    matches!(
        identity,
        "parser-fatal-engine-invariant"
            | "parser-fatal-resource-exhaustion"
            | "parser-invariant"
            | "tokenizer-invariant"
            | "token-canonicalization-invariant"
            | "tree-transition-token-canonicalization-invariant"
            | "unsupported-feature-observation-invariant"
            | "observation-recorder-missing"
            | "patch-history-capture-missing"
            | "observation-invariant"
            | "observation-resource-exhaustion"
    )
}

pub(super) const fn parser_observation_failure_spelling(
    identity: ParserObservationFailureClass,
) -> ParserObservationFailureSpelling {
    match identity {
        I::ParserFatal(ParserFatalIdentity::EngineInvariant) => ParserObservationFailureSpelling {
            identity: "parser-fatal-engine-invariant",
            code: None,
            site: None,
        },
        I::ParserFatal(ParserFatalIdentity::ResourceExhaustion(site)) => {
            ParserObservationFailureSpelling {
                identity: "parser-fatal-resource-exhaustion",
                code: None,
                site: Some(parser_reservation_site_name(site)),
            }
        }
        I::ParserInvariant => ParserObservationFailureSpelling {
            identity: "parser-invariant",
            code: None,
            site: None,
        },
        I::TokenizerInvariant(code) => ParserObservationFailureSpelling {
            identity: "tokenizer-invariant",
            code: Some(tokenizer_invariant_name(code)),
            site: None,
        },
        I::TokenCanonicalizationInvariant => ParserObservationFailureSpelling {
            identity: "token-canonicalization-invariant",
            code: None,
            site: None,
        },
        I::TreeTransitionTokenCanonicalizationInvariant => ParserObservationFailureSpelling {
            identity: "tree-transition-token-canonicalization-invariant",
            code: None,
            site: None,
        },
        I::UnsupportedFeatureObservationInvariant(code) => ParserObservationFailureSpelling {
            identity: "unsupported-feature-observation-invariant",
            code: Some(unsupported_observation_invariant_name(code)),
            site: None,
        },
        I::ObservationRecorderMissing => ParserObservationFailureSpelling {
            identity: "observation-recorder-missing",
            code: None,
            site: None,
        },
        I::PatchHistoryCaptureMissing => ParserObservationFailureSpelling {
            identity: "patch-history-capture-missing",
            code: None,
            site: None,
        },
        I::ObservationInvariant(code) => ParserObservationFailureSpelling {
            identity: "observation-invariant",
            code: Some(observation_invariant_name(code)),
            site: None,
        },
        I::ResourceExhaustion(site) => ParserObservationFailureSpelling {
            identity: "observation-resource-exhaustion",
            code: None,
            site: Some(observation_reservation_site_name(site)),
        },
    }
}

pub(super) fn parser_observation_failure_name(identity: ParserObservationFailureClass) -> String {
    let spelling = parser_observation_failure_spelling(identity);
    match (spelling.code, spelling.site) {
        (Some(code), None) => format!("{}:{code}", spelling.identity),
        (None, Some(site)) => format!("{}:{site}", spelling.identity),
        (None, None) => spelling.identity.to_string(),
        (Some(_), Some(_)) => unreachable!("sealed failure spelling has one owned field"),
    }
}

pub(super) fn parse_runner_invariant(
    value: &str,
) -> Result<ValidatedFixtureInvariantCode, FailureSpellingError> {
    parse_runner_invariant_inner(value).ok_or(FailureSpellingError::UnknownRunnerInvariant)
}

pub(super) fn execution_failure_name(value: ExecutionFailureClass) -> String {
    match value {
        ExecutionFailureClass::SnapshotRead(surface) => {
            format!("snapshot-read:{}", surface.name())
        }
        ExecutionFailureClass::SnapshotFormat(surface) => {
            format!("snapshot-format:{}", surface.name())
        }
        ExecutionFailureClass::ParserObservation(identity) => format!(
            "parser-observation:{}",
            parser_observation_failure_name(identity)
        ),
        ExecutionFailureClass::ValidatedFixtureInvariant(code) => {
            format!("validated-runner-invariant:{}", runner_invariant_name(code))
        }
    }
}
