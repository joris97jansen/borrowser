use super::model::*;
use html::conformance::{
    FinalInvariantRequest, ObservationRequest, ParserObservationRequest, ScalarObservationRequest,
};

/// Private canonical fixture-runner observation guardrails. These are
/// defensive harness policy, not production parser limits, and fixture TOML or
/// sidecars cannot configure them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FixtureObservationGuardrails {
    pub(super) tokens: usize,
    pub(super) parse_errors: usize,
    pub(super) implementation_diagnostics: usize,
    pub(super) unsupported_features: usize,
    pub(super) canonical_tree_units: usize,
    pub(super) transitions: usize,
    pub(super) patch_operations: usize,
}

impl FixtureObservationGuardrails {
    pub(super) const PRODUCTION: Self = Self {
        tokens: 65_536,
        parse_errors: 65_536,
        implementation_diagnostics: 65_536,
        unsupported_features: 65_536,
        canonical_tree_units: 131_072,
        transitions: 262_144,
        patch_operations: 262_144,
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RequestedSurfaces {
    pub(super) tokens: bool,
    pub(super) parse_errors: bool,
    pub(super) implementation_diagnostics: bool,
    pub(super) document_mode: bool,
    pub(super) tree: bool,
    pub(super) patches: bool,
    pub(super) transitions: bool,
    pub(super) unsupported_features: bool,
    pub(super) final_invariants: bool,
}

impl RequestedSurfaces {
    pub(super) fn parity(target: &ValidatedParserTarget) -> Self {
        let document = matches!(target, ValidatedParserTarget::Document { .. });
        Self {
            tokens: true,
            parse_errors: true,
            implementation_diagnostics: true,
            document_mode: document,
            tree: document,
            patches: document,
            transitions: document,
            unsupported_features: true,
            final_invariants: true,
        }
    }
}

pub(super) fn observation_request<'a>(
    target: html::conformance::ParserObservationTarget,
    text: &'a str,
    surfaces: RequestedSurfaces,
    guardrails: FixtureObservationGuardrails,
) -> ParserObservationRequest<'a> {
    ParserObservationRequest {
        target,
        input: html::conformance::ParserObservationInput::Utf8(text),
        tokens: request(surfaces.tokens, guardrails.tokens),
        parse_errors: request(surfaces.parse_errors, guardrails.parse_errors),
        implementation_diagnostics: request(
            surfaces.implementation_diagnostics,
            guardrails.implementation_diagnostics,
        ),
        transitions: request(surfaces.transitions, guardrails.transitions),
        unsupported_features: request(
            surfaces.unsupported_features,
            guardrails.unsupported_features,
        ),
        document_mode: if surfaces.document_mode {
            ScalarObservationRequest::Capture
        } else {
            ScalarObservationRequest::NotRequested
        },
        tree: request(surfaces.tree, guardrails.canonical_tree_units),
        patches: request(surfaces.patches, guardrails.patch_operations),
        final_invariants: if surfaces.final_invariants {
            FinalInvariantRequest::Capture
        } else {
            FinalInvariantRequest::NotRequested
        },
    }
}

pub(super) fn observation_request_for_input<'a>(
    target: html::conformance::ParserObservationTarget,
    input: html::conformance::ParserObservationInput<'a>,
    surfaces: RequestedSurfaces,
    guardrails: FixtureObservationGuardrails,
) -> ParserObservationRequest<'a> {
    let mut request = observation_request(target, "", surfaces, guardrails);
    request.input = input;
    request
}

const fn request(enabled: bool, capacity: usize) -> ObservationRequest {
    if enabled {
        ObservationRequest::Capture { capacity }
    } else {
        ObservationRequest::NotRequested
    }
}
