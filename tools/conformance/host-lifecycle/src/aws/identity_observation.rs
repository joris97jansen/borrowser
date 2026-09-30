//! Pure successor normalization. No reviewed identities, reads, coverage or policy.
use crate::{
    Result,
    provider::{
        identity_observation_v3::*,
        management_observation_v2::{ObservationValueV2, UnavailableEvidenceV2},
        observation::{EvidenceList, ProviderText},
        observation_v3::ObservationDataV3,
    },
};
use aws_sdk_kms::operation::describe_key::DescribeKeyOutput;

#[derive(Debug)]
pub(super) struct NormalizedIdentityV3 {
    pub data: ObservationDataV3,
    /// Supplied for later executor accounting, not an independent/refundable budget.
    pub occurrences: u64,
}
impl NormalizedIdentityV3 {
    pub(super) fn new(data: ObservationDataV3) -> Result<Self> {
        data.validate()?;
        Ok(Self {
            occurrences: data.minimum_occurrences(),
            data,
        })
    }
}
pub(super) fn not_returned<T>() -> ObservationValueV2<T> {
    ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotReturned)
}
fn member<T>(value: Option<&str>, parse: impl FnOnce(&str) -> Option<T>) -> IdentityMemberV3<T> {
    use IdentityMemberV3::*;
    match value {
        None => NotReturned,
        Some("") => Empty,
        Some(v) if v.len() > 2048 => Unrepresentable(MemberRepresentationFailureV3::TextBytes),
        Some(v) if v.contains('\0') => Unrepresentable(MemberRepresentationFailureV3::ContainsNul),
        Some(v) => match parse(v) {
            Some(value) => Present(value),
            None => Malformed(v.to_owned().try_into().expect("bounded non-NUL member")),
        },
    }
}
pub(super) fn identifier<T: std::str::FromStr>(value: Option<&str>) -> IdentityMemberV3<T> {
    member(value, |v| v.parse().ok())
}
fn text(value: Option<&str>) -> IdentityMemberV3<ProviderText> {
    member(value, |v| v.to_owned().try_into().ok())
}
pub(super) fn role_members(arn: Option<&str>, id: Option<&str>) -> IamRoleEvidenceV3 {
    IamRoleEvidenceV3 {
        arn: identifier(arn),
        id: identifier(id),
    }
}
pub(super) fn profile_members(
    arn: Option<&str>,
    id: Option<&str>,
    roles: ObservationValueV2<EvidenceList<IamRoleEvidenceV3>>,
) -> IamProfileEvidenceV3 {
    IamProfileEvidenceV3 {
        arn: identifier(arn),
        id: identifier(id),
        roles,
    }
}
pub(super) fn normalize_key(output: &DescribeKeyOutput) -> Result<NormalizedIdentityV3> {
    let metadata = output.key_metadata().map_or_else(not_returned, |value| {
        ObservationValueV2::Present(KmsKeyEvidenceV3 {
            arn: identifier(value.arn()),
            account: identifier(value.aws_account_id()),
            manager: text(value.key_manager().map(|v| v.as_str())),
            spec: text(value.key_spec().map(|v| v.as_str())),
            usage: text(value.key_usage().map(|v| v.as_str())),
            state: text(value.key_state().map(|v| v.as_str())),
        })
    });
    NormalizedIdentityV3::new(ObservationDataV3::Key { metadata })
}
