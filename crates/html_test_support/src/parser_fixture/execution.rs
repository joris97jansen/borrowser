use super::model::*;
use super::validate::ValidatedFixtureSpec;
use html::conformance::{ObservationRequest, ParserObservationRequest, ScalarObservationRequest};
use std::collections::BTreeSet;

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
}

impl RequestedSurfaces {
    pub(super) fn ordinary(expectations: &EnabledExpectations) -> Self {
        Self {
            tokens: expectations.is_declared(ExpectationSurface::Tokens),
            parse_errors: expectations.is_declared(ExpectationSurface::ParseErrors),
            implementation_diagnostics: expectations
                .is_declared(ExpectationSurface::ImplementationDiagnostics),
            document_mode: expectations.is_declared(ExpectationSurface::DocumentMode),
            tree: expectations.is_declared(ExpectationSurface::Tree),
            patches: expectations.is_declared(ExpectationSurface::Patches),
            transitions: false,
            unsupported_features: expectations.is_declared(ExpectationSurface::UnsupportedFeatures),
        }
    }

    fn union(&mut self, other: Self) {
        self.tokens |= other.tokens;
        self.parse_errors |= other.parse_errors;
        self.implementation_diagnostics |= other.implementation_diagnostics;
        self.document_mode |= other.document_mode;
        self.tree |= other.tree;
        self.patches |= other.patches;
        self.transitions |= other.transitions;
        self.unsupported_features |= other.unsupported_features;
    }

    pub(super) fn any(self) -> bool {
        self.tokens
            || self.parse_errors
            || self.implementation_diagnostics
            || self.document_mode
            || self.tree
            || self.patches
            || self.transitions
            || self.unsupported_features
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PlannedDelivery {
    pub(super) name: DeliveryName,
    pub(super) surfaces: RequestedSurfaces,
}

pub(super) fn build_delivery_plan(
    fixture: &ValidatedFixtureSpec,
) -> Result<Vec<PlannedDelivery>, ValidatedFixtureInvariantCode> {
    let ordinary = RequestedSurfaces::ordinary(fixture.expectations());
    let mut requested = Vec::<(DeliveryName, RequestedSurfaces)>::new();
    if ordinary.any() {
        requested.push((fixture.execution().reference_delivery().clone(), ordinary));
    }
    if let ExpectedSurface::Compare(transitions) = fixture.expectations().transitions() {
        for transition in transitions {
            requested.push((
                transition.delivery().clone(),
                RequestedSurfaces {
                    transitions: true,
                    ..RequestedSurfaces::default()
                },
            ));
        }
    }

    let mut plan = Vec::new();
    for delivery in fixture.execution().deliveries() {
        let mut surfaces = RequestedSurfaces::default();
        for (name, requested_surfaces) in &requested {
            if name == delivery.name() {
                surfaces.union(*requested_surfaces);
            }
        }
        if surfaces.any() {
            if plan
                .iter()
                .any(|planned: &PlannedDelivery| planned.name == *delivery.name())
            {
                return Err(ValidatedFixtureInvariantCode::DuplicatePlannedDelivery);
            }
            plan.push(PlannedDelivery {
                name: delivery.name().clone(),
                surfaces,
            });
        }
    }
    for (name, _) in requested {
        if !plan.iter().any(|planned| planned.name == name) {
            return Err(if name == *fixture.execution().reference_delivery() {
                ValidatedFixtureInvariantCode::PlannedReferenceDeliveryMissing
            } else {
                ValidatedFixtureInvariantCode::PlannedDeliveryMissing
            });
        }
    }
    let unique = plan
        .iter()
        .map(|planned| planned.name.clone())
        .collect::<BTreeSet<_>>();
    if unique.len() != plan.len() {
        return Err(ValidatedFixtureInvariantCode::DuplicatePlannedDelivery);
    }
    Ok(plan)
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
    }
}

const fn request(enabled: bool, capacity: usize) -> ObservationRequest {
    if enabled {
        ObservationRequest::Capture { capacity }
    } else {
        ObservationRequest::NotRequested
    }
}
