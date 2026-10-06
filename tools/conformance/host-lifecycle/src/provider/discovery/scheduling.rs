//! Deterministic seed stages, then breadth-first expansion from supplied provenance.
use super::{relationships::*, *};
use crate::provider::{
    coverage::QueryScopeV1 as S, inventory::ResourceIdentity as R,
    reviewed_subnet_routes_v1::DiscoveryQuery,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkStage {
    ClientToken,
    AuthorityTags,
    OperationTags,
    ReviewedInfrastructure,
    PriorIdentities,
    Relationships { depth: u16 },
}
impl WorkStage {
    pub(super) fn seed(query: &DiscoveryQuery) -> Self {
        match query {
            DiscoveryQuery::Existing(q) => match q.scope {
                S::ClientToken { .. } => Self::ClientToken,
                S::AuthorityTag { .. } => Self::AuthorityTags,
                S::OperationTag { .. } => Self::OperationTags,
                _ => Self::ReviewedInfrastructure,
            },
            _ => Self::ReviewedInfrastructure,
        }
    }
}
fn target(scope: &S) -> Option<AllocationIdentity> {
    use AllocationIdentity as I;
    match scope {
        S::AttachedTo { instance } | S::InstanceAttribute { instance, .. } => {
            Some(I::Instance(instance.clone()))
        }
        S::Exact { identities } if identities.len() == 1 => match &identities[0] {
            R::Instance(v) => Some(I::Instance(v.clone())),
            R::NetworkInterface(v) => Some(I::NetworkInterface(v.clone())),
            R::Volume(v) => Some(I::Volume(v.clone())),
            R::ProfileAssociation(v) => Some(I::ProfileAssociation(v.clone())),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn derive_depths(
    inputs: &DiscoveryInputs<'_>,
    evidence: &ValidatedDiscoveryEvidence,
    graph: &mut Relationships,
) {
    // Repeated relaxation uses only retained references and immutable query/source
    // provenance. It requires no coordinator history and allocates no depth index.
    // Positional parents and children are observed in that same query: containment
    // does not add an SDK-read hop, nor promote a child to a prior parent's depth
    // zero. Keep the depth attached to each actual source observation.
    let binding = &inputs.context.fields().binding;
    loop {
        let mut changed = false;
        for index in 0..evidence.evidence.observations.records.len() {
            let query = evidence.evidence.observations.records[index].query();
            let seed = match &query.scope {
                S::ClientToken { token } => token == &binding.client_token,
                S::AuthorityTag { authority } => authority == &binding.authority_id,
                S::OperationTag { operation } | S::OperationTags { operation, .. } => {
                    operation == &binding.operation_id
                }
                _ => false,
            };
            let target = target(&query.scope);
            let depth = if seed {
                Some(1)
            } else {
                target
                    .as_ref()
                    .and_then(|id| graph.resources.get(id))
                    .and_then(|r| r.discovery_depth)
                    .and_then(|d| d.checked_add(1))
            };
            if let Some(depth) = depth {
                for node in graph.resources.values_mut() {
                    if node.observations.contains(&index)
                        && node.discovery_depth.is_none_or(|old| depth < old)
                    {
                        node.discovery_depth = Some(depth);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}
