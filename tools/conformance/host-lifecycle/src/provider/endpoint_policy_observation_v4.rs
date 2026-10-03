//! Fixed-depth endpoint-policy occurrences. No unique-key maps or effective-policy semantics.
use super::{
    coverage::ReadFailureV1,
    ec2_observation_v4::*,
    identity_observation_v3::MemberRepresentationFailureV3,
    limits::LimitKind,
    observation::{EvidenceList, ProviderText},
};
use crate::{Result, canonical, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyShapeV4 {
    Null,
    Boolean,
    Number,
    String,
    Array,
    Object,
}
impl FactsV4 for PolicyShapeV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4 {
            occurrences: 0,
            failure: Some(ReadFailureV1::Unsupported),
        })
    }
}
macro_rules! policy_enum {
    ($name:ident { $($variant:ident($ty:ty)),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag="kind", content="value", rename_all="kebab-case", deny_unknown_fields)]
        pub enum $name { $($variant($ty)),* }
        impl FactsV4 for $name {
            fn facts(&self) -> Result<FactSummaryV4> {
                match self { $(Self::$variant(value) => value.facts()),* }
            }
        }
    };
}
impl<T: FactsV4> FactsV4 for Box<T> {
    fn facts(&self) -> Result<FactSummaryV4> {
        (**self).facts()
    }
}
// An unknown name is retained with a closed value-shape marker, never its recursive payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsupportedPolicyMemberV4 {
    pub name: Ec2MemberV4<ProviderText>,
    pub shape: PolicyShapeV4,
}
impl FactsV4 for UnsupportedPolicyMemberV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        let mut summary = self.name.facts()?;
        summary.merge(self.shape.facts()?)?;
        Ok(summary)
    }
}
// Scalar objects, arrays and closed unsupported-shape strings are disjoint. These
// containers add no redundant wrapper to the frozen encoder's depth-16 budget.
macro_rules! policy_container {
    ($name:ident { $($variant:ident($ty:ty)),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(untagged)]
        pub enum $name { $($variant($ty)),* }
        impl FactsV4 for $name {
            fn facts(&self) -> Result<FactSummaryV4> {match self {$(Self::$variant(v)=>v.facts()),*}}
        }
    };
}
policy_container!(PolicyTextV4 { Text(Ec2MemberV4<ProviderText>), Unsupported(PolicyShapeV4) });
policy_container!(PolicyLiteralV4 { Text(Ec2MemberV4<ProviderText>), Boolean(bool), Unsupported(PolicyShapeV4) });
policy_container!(PolicyStringsV4 { Scalar(PolicyTextV4), Array(EvidenceList<PolicyTextV4>) });
impl PolicyStringsV4 {
    pub(crate) fn value_count(&self) -> usize {
        match self {
            Self::Array(v) => v.as_slice().len(),
            _ => 1,
        }
    }
}
policy_container!(PolicyValuesV4 { Scalar(PolicyLiteralV4), Array(EvidenceList<PolicyLiteralV4>) });
fact_struct!(PolicyPrincipalEntryV4 { principal: Ec2MemberV4<ProviderText>, identities: PolicyStringsV4 });
policy_enum!(PolicyPrincipalsV4 { Literal(PolicyTextV4), Entries(EvidenceList<PolicyPrincipalEntryV4>), Unsupported(PolicyShapeV4) });
impl PolicyPrincipalsV4 {
    fn counts(&self) -> (usize, usize) {
        match self {
            Self::Entries(v) => (
                v.as_slice().len(),
                v.as_slice()
                    .iter()
                    .map(|v| v.identities.value_count())
                    .sum(),
            ),
            _ => (0, 1),
        }
    }
}
fact_struct!(PolicyConditionV4 { key: Ec2MemberV4<ProviderText>, values: PolicyValuesV4 });
policy_container!(PolicyConditionEntriesV4 { Entries(EvidenceList<PolicyConditionV4>), Unsupported(PolicyShapeV4) });
fact_struct!(PolicyOperatorV4 { operator: Ec2MemberV4<ProviderText>, conditions: PolicyConditionEntriesV4 });
policy_container!(PolicyConditionsV4 { Operators(EvidenceList<PolicyOperatorV4>), Unsupported(PolicyShapeV4) });
impl PolicyConditionsV4 {
    fn count(&self) -> usize {
        match self {
            Self::Operators(v) => v
                .as_slice()
                .iter()
                .map(|v| match &v.conditions {
                    PolicyConditionEntriesV4::Entries(v) => v.as_slice().len(),
                    _ => 1,
                })
                .sum(),
            _ => 1,
        }
    }
}
policy_enum!(PolicyStatementMemberV4 {
    Sid(PolicyTextV4), Effect(PolicyTextV4), Principal(PolicyPrincipalsV4),
    NotPrincipal(PolicyPrincipalsV4), Action(PolicyStringsV4), NotAction(PolicyStringsV4),
    Resource(PolicyStringsV4), NotResource(PolicyStringsV4), Condition(PolicyConditionsV4),
    Unsupported(UnsupportedPolicyMemberV4),
});
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyStatementV4 {
    pub members: EvidenceList<PolicyStatementMemberV4>,
}
impl FactsV4 for PolicyStatementV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        let (mut entries, mut identities, mut actions, mut resources, mut conditions) =
            (0, 0, 0, 0, 0);
        for member in self.members.as_slice() {
            match member {
                PolicyStatementMemberV4::Sid(PolicyTextV4::Text(Ec2MemberV4::Present(v))) => {
                    require(v.as_str().len() <= 128, "policy Sid bound")?
                }
                PolicyStatementMemberV4::Principal(v)
                | PolicyStatementMemberV4::NotPrincipal(v) => {
                    let (e, i) = v.counts();
                    entries += e;
                    identities += i;
                }
                PolicyStatementMemberV4::Action(v) | PolicyStatementMemberV4::NotAction(v) => {
                    actions += v.value_count()
                }
                PolicyStatementMemberV4::Resource(v) | PolicyStatementMemberV4::NotResource(v) => {
                    resources += v.value_count()
                }
                PolicyStatementMemberV4::Condition(v) => conditions += v.count(),
                _ => (),
            }
        }
        require(
            entries <= 16 && identities <= 16 && actions <= 8 && resources <= 16 && conditions <= 8,
            "aggregate policy member bound",
        )?;
        self.members.facts()
    }
}
policy_container!(PolicyStatementValueV4 { Object(PolicyStatementV4), Unsupported(PolicyShapeV4) });
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PolicyStatementsV4 {
    Object(PolicyStatementV4),
    Array(EvidenceList<PolicyStatementValueV4>),
    Unsupported(PolicyShapeV4),
}
impl PolicyStatementsV4 {
    fn count(&self) -> usize {
        match self {
            Self::Array(v) => v.as_slice().len(),
            _ => 1,
        }
    }
}
impl FactsV4 for PolicyStatementsV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        match self {
            Self::Array(v) => v.facts(),
            Self::Object(v) => {
                let mut s = v.facts()?;
                s.occurrences += 1;
                Ok(s)
            }
            Self::Unsupported(v) => {
                let mut s = v.facts()?;
                s.occurrences += 1;
                Ok(s)
            }
        }
    }
}
policy_enum!(PolicyDocumentMemberV4 {
    Version(PolicyTextV4), Id(PolicyTextV4), Statement(PolicyStatementsV4), Unsupported(UnsupportedPolicyMemberV4),
});
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyDocumentV4 {
    pub members: EvidenceList<PolicyDocumentMemberV4>,
}
impl FactsV4 for PolicyDocumentV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        let count: usize = self
            .members
            .as_slice()
            .iter()
            .map(|v| match v {
                PolicyDocumentMemberV4::Statement(v) => v.count(),
                _ => 0,
            })
            .sum();
        require(count <= 16, "aggregate policy statement bound")?;
        require(
            canonical::encode(self)?.len() <= 8192,
            "observed policy bytes",
        )?;
        self.members.facts()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum EndpointPolicyValueV4 {
    NotReturned,
    Empty,
    Document(PolicyDocumentV4),
    Malformed,
    Unsupported(PolicyShapeV4),
    Unrepresentable(MemberRepresentationFailureV3),
}
impl FactsV4 for EndpointPolicyValueV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        let failure = match self {
            Self::Document(v) => return v.facts(),
            Self::Unsupported(v) => return v.facts(),
            Self::Empty
            | Self::Malformed
            | Self::Unrepresentable(MemberRepresentationFailureV3::ContainsNul) => {
                Some(ReadFailureV1::Malformed)
            }
            Self::Unrepresentable(MemberRepresentationFailureV3::TextBytes) => {
                Some(ReadFailureV1::Limit(LimitKind::RecordBytes))
            }
            Self::NotReturned => None,
        };
        Ok(FactSummaryV4 {
            occurrences: 0,
            failure,
        })
    }
}
