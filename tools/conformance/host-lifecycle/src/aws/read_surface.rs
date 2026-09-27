//! Closed audit inventory. Does not dispatch operations or normalize responses.
use crate::provider::coverage::ReadOperationV1;

pub(super) const READ_SURFACE: &[ReadOperationV1] = &[
    ReadOperationV1::GetCallerIdentity,
    ReadOperationV1::HeadBucket,
    ReadOperationV1::GetInstanceProfile,
    ReadOperationV1::DescribeKey,
    ReadOperationV1::DescribeRegions,
    ReadOperationV1::DescribeAvailabilityZones,
    ReadOperationV1::DescribeSubnets,
    ReadOperationV1::DescribeVpcs,
    ReadOperationV1::DescribeSecurityGroups,
    ReadOperationV1::DescribeRouteTables,
    ReadOperationV1::DescribeVpcEndpoints,
    ReadOperationV1::DescribePrefixLists,
    ReadOperationV1::DescribeVpcAttribute,
    ReadOperationV1::DescribeDhcpOptions,
    ReadOperationV1::DescribeNetworkAcls,
    ReadOperationV1::DescribeImages,
    ReadOperationV1::DescribeInstanceTypes,
    ReadOperationV1::DescribeInstanceTypeOfferings,
    ReadOperationV1::DescribeIamInstanceProfileAssociations,
    ReadOperationV1::DescribeInstances,
    ReadOperationV1::DescribeNetworkInterfaces,
    ReadOperationV1::DescribeVolumes,
    ReadOperationV1::DescribeInstanceAttribute,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_independent_read_inventory() {
        let expected: Vec<ReadOperationV1> = serde_json::from_str(include_str!(
            "../../tests/fixtures/read-sdk-surface-v1.json"
        ))
        .unwrap();
        assert_eq!(READ_SURFACE, expected);
        for action in [
            "RunInstances",
            "TerminateInstances",
            "GetRole",
            "AssumeRole",
            "CreateSession",
            "GetObject",
            "ListKeys",
            "Encrypt",
        ] {
            assert!(serde_json::from_value::<ReadOperationV1>(action.into()).is_err());
        }
    }
}
