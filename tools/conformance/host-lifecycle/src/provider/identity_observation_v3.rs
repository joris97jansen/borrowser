//! Partial identity evidence. No requested identity, policy or SDK types enter here.
use super::{
    management_observation_v2::ObservationValueV2,
    observation::{EvidenceList, ProviderText},
};
use crate::{Result, identity::*, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum IdentityMemberV3<T> {
    NotReturned,
    Empty,
    Present(T),
    Malformed(ProviderText),
    Unrepresentable(MemberRepresentationFailureV3),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemberRepresentationFailureV3 {
    TextBytes,
    ContainsNul,
}

impl<T: std::str::FromStr> IdentityMemberV3<T> {
    fn validate_identifier(&self) -> Result<()> {
        if let Self::Malformed(value) = self {
            require(
                !value.as_str().is_empty() && value.as_str().parse::<T>().is_err(),
                "malformed identity member representation",
            )?;
        }
        Ok(())
    }
}
impl IdentityMemberV3<ProviderText> {
    fn validate_text(&self) -> Result<()> {
        match self {
            Self::Present(value) => {
                require(!value.as_str().is_empty(), "empty member representation")
            }
            Self::Malformed(_) => Err(crate::Error("literal member cannot be malformed identity")),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KmsKeyEvidenceV3 {
    pub arn: IdentityMemberV3<KmsKeyArn>,
    pub account: IdentityMemberV3<AwsAccountId>,
    pub manager: IdentityMemberV3<ProviderText>,
    pub spec: IdentityMemberV3<ProviderText>,
    pub usage: IdentityMemberV3<ProviderText>,
    pub state: IdentityMemberV3<ProviderText>,
}
impl KmsKeyEvidenceV3 {
    pub fn validate(&self) -> Result<()> {
        self.arn.validate_identifier()?;
        self.account.validate_identifier()?;
        for field in [&self.manager, &self.spec, &self.usage, &self.state] {
            field.validate_text()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IamRoleEvidenceV3 {
    pub arn: IdentityMemberV3<IamRoleArn>,
    pub id: IdentityMemberV3<IamRoleId>,
}
impl IamRoleEvidenceV3 {
    pub fn validate(&self) -> Result<()> {
        self.arn.validate_identifier()?;
        self.id.validate_identifier()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IamProfileEvidenceV3 {
    pub arn: IdentityMemberV3<InstanceProfileArn>,
    pub id: IdentityMemberV3<InstanceProfileId>,
    pub roles: ObservationValueV2<EvidenceList<IamRoleEvidenceV3>>,
}
impl IamProfileEvidenceV3 {
    pub fn validate(&self) -> Result<()> {
        self.arn.validate_identifier()?;
        self.id.validate_identifier()?;
        if let ObservationValueV2::Present(roles) = &self.roles {
            for role in roles.as_slice() {
                role.validate()?;
            }
        }
        Ok(())
    }
}
