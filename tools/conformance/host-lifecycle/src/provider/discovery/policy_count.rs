//! Exact supported endpoint-policy JSON scanner work, not historical minimum credit.
use crate::provider::endpoint_policy_observation_v4::*;

fn strings(v: &PolicyStringsV4) -> Option<u64> {
    match v {
        PolicyStringsV4::Scalar(v) => text(v),
        PolicyStringsV4::Array(v) => v.as_slice().iter().try_fold(1, |n, v| Some(n + text(v)?)),
    }
}
fn text(v: &PolicyTextV4) -> Option<u64> {
    match v {
        PolicyTextV4::Text(_) => Some(1),
        _ => None,
    }
}
fn literal(v: &PolicyLiteralV4) -> Option<u64> {
    match v {
        PolicyLiteralV4::Text(_) | PolicyLiteralV4::Boolean(_) => Some(1),
        _ => None,
    }
}
fn values(v: &PolicyValuesV4) -> Option<u64> {
    match v {
        PolicyValuesV4::Scalar(v) => literal(v),
        PolicyValuesV4::Array(v) => v
            .as_slice()
            .iter()
            .try_fold(1, |n, v| Some(n + literal(v)?)),
    }
}
fn principal(v: &PolicyPrincipalsV4) -> Option<u64> {
    match v {
        PolicyPrincipalsV4::Literal(v) => text(v),
        PolicyPrincipalsV4::Entries(v) => v
            .as_slice()
            .iter()
            .try_fold(1, |n, v| Some(n + 1 + strings(&v.identities)?)),
        _ => None,
    }
}
fn condition(v: &PolicyConditionsV4) -> Option<u64> {
    let PolicyConditionsV4::Operators(v) = v else {
        return None;
    };
    v.as_slice().iter().try_fold(1, |n, v| {
        let PolicyConditionEntriesV4::Entries(v) = &v.conditions else {
            return None;
        };
        Some(
            n + 1
                + v.as_slice()
                    .iter()
                    .try_fold(1, |n, v| Some(n + 1 + values(&v.values)?))?,
        )
    })
}
fn statement(v: &PolicyStatementV4) -> Option<u64> {
    v.members.as_slice().iter().try_fold(1, |n, v| {
        use PolicyStatementMemberV4 as M;
        Some(
            n + 1
                + match v {
                    M::Sid(v) | M::Effect(v) => text(v)?,
                    M::Principal(v) | M::NotPrincipal(v) => principal(v)?,
                    M::Action(v) | M::NotAction(v) | M::Resource(v) | M::NotResource(v) => {
                        strings(v)?
                    }
                    M::Condition(v) => condition(v)?,
                    M::Unsupported(_) => return None,
                },
        )
    })
}
pub(super) fn count(v: &EndpointPolicyValueV4) -> Option<u64> {
    match v {
        EndpointPolicyValueV4::NotReturned => Some(0),
        EndpointPolicyValueV4::Document(v) => v.members.as_slice().iter().try_fold(1, |n, v| {
            Some(
                n + 1
                    + match v {
                        PolicyDocumentMemberV4::Version(v) | PolicyDocumentMemberV4::Id(v) => {
                            text(v)?
                        }
                        PolicyDocumentMemberV4::Statement(PolicyStatementsV4::Object(v)) => {
                            statement(v)?
                        }
                        PolicyDocumentMemberV4::Statement(PolicyStatementsV4::Array(v)) => {
                            v.as_slice().iter().try_fold(1, |n, v| match v {
                                PolicyStatementValueV4::Object(v) => Some(n + statement(v)?),
                                _ => None,
                            })?
                        }
                        _ => return None,
                    },
            )
        }),
        _ => None,
    }
}
