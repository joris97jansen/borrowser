//! Live read coordination over the pure required-work frontier. No admission or binding.
use super::observation_session::ObservationSession;
use crate::{
    Result,
    provider::{
        discovery::*,
        observation_v5::ProviderObservationV5,
        reviewed_subnet_routes_v1::{DiscoveryQuery, ReviewedSubnetRouteEvidenceV1},
    },
};

pub(super) struct ObservedDiscovery {
    pub evidence: ValidatedDiscoveryEvidence,
    pub execution: DiscoveryExecutionFacts,
    pub report: DiscoveryReport,
}
pub(super) async fn observe(
    session: &mut ObservationSession,
    inputs: &DiscoveryInputs<'_>,
) -> Result<ObservedDiscovery> {
    crate::require(
        session.deployment() == &inputs.prepared.deployment.digest()?,
        "discovery session binding",
    )?;
    let context = inputs.context_identity()?;
    let mut evidence = DiscoveryEvidence {
        observations: ProviderObservationV5 {
            context: context.clone(),
            records: Vec::new(),
            coverage: Vec::new(),
        },
        reviewed_subnet_routes: None,
    };
    let mut rejected = Vec::new();
    loop {
        let validated = ingest(inputs, evidence).map_err(|e| e.error)?;
        let facts = DiscoveryExecutionFacts {
            accounting: session.round().snapshot(),
            rejected_before_admission: rejected.clone(),
            stop: CoordinatorStop::Deriving,
            final_check: FinalRoundCheck::Pending,
        };
        let preview = derive_offline(inputs, &validated, &facts)?;
        let exhausted = preview.metadata_exhausted;
        // The pure report owns the same stage/depth ordering used offline.
        let next = preview.next_pending().map(|v| v.query.clone());
        if exhausted || next.is_none() || session.round().failure().is_some() {
            let stop = if exhausted {
                CoordinatorStop::MetadataExhausted
            } else if session.round().failure().is_some() {
                CoordinatorStop::RoundStopped
            } else {
                CoordinatorStop::Quiescent
            };
            let mut execution = DiscoveryExecutionFacts {
                accounting: session.round().snapshot(),
                rejected_before_admission: rejected,
                stop,
                final_check: FinalRoundCheck::Pending,
            };
            let prepared = derive(inputs, &validated, &execution)?;
            // This is deliberately after traversal, representation audit, sorting,
            // report preparation and exclusion preparation, including a zero-I/O tail.
            let final_check = match session.round().remaining() {
                Ok(_) => FinalRoundCheck::Passed,
                Err(_) => FinalRoundCheck::Failed(
                    session
                        .round()
                        .failure()
                        .unwrap_or(crate::provider::limits::LimitKind::Clock),
                ),
            };
            execution.accounting = session.round().snapshot();
            execution.final_check = final_check;
            let report = prepared.finalize(final_check);
            return Ok(ObservedDiscovery {
                evidence: validated,
                execution,
                report,
            });
        }
        evidence = validated.into_evidence();
        let next = next.expect("frontier work");
        let result = match &next {
            DiscoveryQuery::Existing(q) => session.observe_query(q).await.map(|r| {
                evidence.observations.records.extend(r.records);
                evidence.observations.coverage.push(r.coverage);
            }),
            DiscoveryQuery::ReviewedSubnetRoutes(q) => {
                if session.route_query() != q {
                    Err(crate::provider::coverage::ReadFailureV1::Unsupported)
                } else {
                    session.reviewed_subnet_routes().await.map(|r| {
                        evidence.reviewed_subnet_routes = Some(ReviewedSubnetRouteEvidenceV1 {
                            context: context.clone(),
                            records: r.records,
                            coverage: r.coverage,
                        });
                    })
                }
            }
        };
        if let Err(reason) = result {
            rejected.push(RejectedRead {
                query: next,
                reason,
            });
        }
    }
}
