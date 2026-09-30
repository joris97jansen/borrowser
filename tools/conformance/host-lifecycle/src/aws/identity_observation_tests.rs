use super::{
    configuration,
    iam_presence::capture_instance_profile,
    identity_observation::*,
    response_limits::{BoundedHttp, ObservationRound},
    tests::{replay, response, secret},
};
use crate::provider::{
    identity_observation_v3::*, management_observation_v2::ObservationValueV2,
    observation_v3::ObservationDataV3,
};
use aws_smithy_runtime_api::client::http::SharedHttpClient;

pub(super) fn iam_xml(profile: &str) -> String {
    format!(
        "<GetInstanceProfileResponse xmlns=\"https://iam.amazonaws.com/doc/2010-05-08/\"><GetInstanceProfileResult>{profile}</GetInstanceProfileResult></GetInstanceProfileResponse>"
    )
}
fn config(body: &str, round: ObservationRound) -> aws_types::SdkConfig {
    configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(
            SharedHttpClient::new(replay(vec![response(200, body, None)])),
            round,
        ),
    )
    .unwrap()
}
pub(super) async fn iam(
    body: &str,
) -> (
    aws_sdk_iam::operation::get_instance_profile::GetInstanceProfileOutput,
    NormalizedIdentityV3,
) {
    let round = ObservationRound::test();
    let conf = config(body, round.clone());
    let client = aws_sdk_iam::Client::from_conf(aws_sdk_iam::config::Builder::from(&conf).build());
    let (capture, receiver) = capture_instance_profile(round);
    let sdk = client
        .get_instance_profile()
        .instance_profile_name("reviewed")
        .customize()
        .interceptor(capture)
        .send()
        .await
        .unwrap();
    (sdk, receiver.take().unwrap())
}
fn profile(value: NormalizedIdentityV3) -> IamProfileEvidenceV3 {
    let ObservationDataV3::Profile {
        profile: ObservationValueV2::Present(value),
    } = value.data
    else {
        panic!()
    };
    value
}
fn key(value: NormalizedIdentityV3) -> KmsKeyEvidenceV3 {
    let ObservationDataV3::Key {
        metadata: ObservationValueV2::Present(value),
    } = value.data
    else {
        panic!()
    };
    value
}
async fn kms(metadata: serde_json::Value) -> NormalizedIdentityV3 {
    let body = serde_json::json!({"KeyMetadata":metadata}).to_string();
    let conf = config(&body, ObservationRound::test());
    let client = aws_sdk_kms::Client::from_conf(aws_sdk_kms::config::Builder::from(&conf).build());
    let sdk = client
        .describe_key()
        .key_id("arn:aws:kms:eu-central-1:111111111111:key/00000000-0000-0000-0000-000000000001")
        .send()
        .await
        .unwrap();
    normalize_key(&sdk).unwrap()
}

#[tokio::test]
async fn kms_independent_identity_states_preserve_contradictory_metadata() {
    for arn in [
        None,
        Some(""),
        Some("malformed"),
        Some("arn:aws:kms:eu-central-1:222222222222:key/00000000-0000-0000-0000-000000000002"),
    ] {
        let mut metadata = serde_json::json!({"KeyId":"returned", "AWSAccountId":"222222222222","KeyManager":"AWS","KeySpec":"FUTURE_SPEC","KeyUsage":"SIGN_VERIFY","KeyState":"Disabled"});
        if let Some(arn) = arn {
            metadata["Arn"] = arn.into();
        }
        let v = key(kms(metadata).await);
        assert_eq!(
            v.account,
            IdentityMemberV3::Present("222222222222".parse().unwrap())
        );
        assert!(matches!(v.state, IdentityMemberV3::Present(_)));
        assert!(matches!(v.spec, IdentityMemberV3::Present(_)));
        match arn {
            None => assert_eq!(v.arn, IdentityMemberV3::NotReturned),
            Some("") => assert_eq!(v.arn, IdentityMemberV3::Empty),
            Some("malformed") => assert!(matches!(v.arn, IdentityMemberV3::Malformed(_))),
            _ => assert!(matches!(v.arn, IdentityMemberV3::Present(_))),
        }
    }
    let v = key(kms(serde_json::json!({"Arn":"x".repeat(2049),"AWSAccountId":"222222222222","KeyState":"Disabled"})).await);
    assert_eq!(
        v.arn,
        IdentityMemberV3::Unrepresentable(MemberRepresentationFailureV3::TextBytes)
    );
    assert!(matches!(v.account, IdentityMemberV3::Present(_)));
    let v = key(kms(
        serde_json::json!({"Arn":"x\u{0000}","AWSAccountId":"bad-account","KeyState":""}),
    )
    .await);
    assert_eq!(
        v.arn,
        IdentityMemberV3::Unrepresentable(MemberRepresentationFailureV3::ContainsNul)
    );
    assert!(matches!(v.account, IdentityMemberV3::Malformed(_)));
    assert_eq!(v.state, IdentityMemberV3::Empty);
    let v = kms(serde_json::Value::Null).await;
    assert!(matches!(
        v.data,
        ObservationDataV3::Key {
            metadata: ObservationValueV2::Unavailable(_)
        }
    ));
}

#[tokio::test]
async fn kms_complete_matching_and_missing_members_use_only_sdk_values() {
    let arn = "arn:aws:kms:eu-central-1:111111111111:key/00000000-0000-0000-0000-000000000001";
    let v=key(kms(serde_json::json!({"Arn":arn,"AWSAccountId":"111111111111","KeyManager":"CUSTOMER","KeySpec":"SYMMETRIC_DEFAULT","KeyUsage":"ENCRYPT_DECRYPT","KeyState":"Enabled"})).await);
    assert_eq!(v.arn, IdentityMemberV3::Present(arn.parse().unwrap()));
    assert_eq!(
        v.account,
        IdentityMemberV3::Present("111111111111".parse().unwrap())
    );
    let v = key(kms(serde_json::json!({})).await);
    assert_eq!(v.arn, IdentityMemberV3::NotReturned);
    assert_eq!(v.account, IdentityMemberV3::NotReturned);
    for field in [v.manager, v.spec, v.usage, v.state] {
        assert_eq!(field, IdentityMemberV3::NotReturned);
    }
    let v = key(kms(serde_json::json!({"Arn":"x".repeat(2048)})).await);
    assert!(matches!(v.arn, IdentityMemberV3::Malformed(_)));
}

#[tokio::test]
async fn iam_complete_roles_and_oversized_member_keep_siblings() {
    let arn = "arn:aws:iam::111111111111:role/reviewed";
    let (_,v)=iam(&iam_xml(&format!("<InstanceProfile><Arn>{}</Arn><InstanceProfileId>AIPA00000000000000000</InstanceProfileId><Roles><member><Arn>{arn}</Arn><RoleId>AROA00000000000000000</RoleId></member><member><Arn>{}</Arn><RoleId>AROA00000000000000001</RoleId></member></Roles></InstanceProfile>","x".repeat(2049),"x".repeat(2049)))).await;
    let p = profile(v);
    assert_eq!(
        p.arn,
        IdentityMemberV3::Unrepresentable(MemberRepresentationFailureV3::TextBytes)
    );
    assert!(matches!(p.id, IdentityMemberV3::Present(_)));
    let ObservationValueV2::Present(roles) = p.roles else {
        panic!()
    };
    assert_eq!(roles.as_slice().len(), 2);
    assert_eq!(
        roles.as_slice()[0].arn,
        IdentityMemberV3::Present(arn.parse().unwrap())
    );
    assert!(matches!(
        roles.as_slice()[0].id,
        IdentityMemberV3::Present(_)
    ));
    assert_eq!(
        roles.as_slice()[1].arn,
        IdentityMemberV3::Unrepresentable(MemberRepresentationFailureV3::TextBytes)
    );
    assert!(matches!(
        roles.as_slice()[1].id,
        IdentityMemberV3::Present(_)
    ));
}
#[tokio::test]
async fn iam_generated_defaults_do_not_erase_omission_or_empty() {
    let (missing_sdk, missing) = iam(&iam_xml("<InstanceProfile/>")).await;
    let (empty_sdk, empty) = iam(&iam_xml(
        "<InstanceProfile><Arn/><InstanceProfileId/><Roles/></InstanceProfile>",
    ))
    .await;
    assert_eq!(
        missing_sdk.instance_profile().unwrap().arn(),
        empty_sdk.instance_profile().unwrap().arn()
    );
    assert!(missing_sdk.instance_profile().unwrap().roles().is_empty());
    assert!(empty_sdk.instance_profile().unwrap().roles().is_empty());
    let missing = profile(missing);
    let empty = profile(empty);
    assert_eq!(missing.arn, IdentityMemberV3::NotReturned);
    assert_eq!(empty.arn, IdentityMemberV3::Empty);
    assert_eq!(missing.id, IdentityMemberV3::NotReturned);
    assert_eq!(empty.id, IdentityMemberV3::Empty);
    assert!(matches!(missing.roles, ObservationValueV2::Unavailable(_)));
    assert!(matches!(empty.roles, ObservationValueV2::Present(_)));
    let (sdk, absent) = iam(&iam_xml("")).await;
    assert!(sdk.instance_profile().is_some());
    assert!(matches!(
        absent.data,
        ObservationDataV3::Profile {
            profile: ObservationValueV2::Unavailable(_)
        }
    ));
}
#[tokio::test]
async fn iam_partial_profile_and_role_members_survive_independently() {
    let foreign = "arn:aws:iam::222222222222:role/foreign";
    for arn in [
        "",
        "<Arn/>",
        "<Arn>bad</Arn>",
        "<Arn>arn:aws:iam::222222222222:instance-profile/foreign</Arn>",
    ] {
        let body = iam_xml(&format!(
            "<InstanceProfile>{arn}<InstanceProfileId>AIPA00000000000000000</InstanceProfileId><Roles><member><Arn>{foreign}</Arn></member></Roles></InstanceProfile>"
        ));
        let (_, v) = iam(&body).await;
        assert_eq!(v.occurrences, 2);
        let p = profile(v);
        assert!(matches!(p.id, IdentityMemberV3::Present(_)));
        let ObservationValueV2::Present(roles) = p.roles else {
            panic!()
        };
        assert_eq!(
            roles.as_slice()[0].arn,
            IdentityMemberV3::Present(foreign.parse().unwrap())
        );
        assert_eq!(roles.as_slice()[0].id, IdentityMemberV3::NotReturned);
    }
    for id in [
        "",
        "<InstanceProfileId/>",
        "<InstanceProfileId>bad</InstanceProfileId>",
    ] {
        let (_, v) = iam(&iam_xml(&format!("<InstanceProfile><Arn>arn:aws:iam::222222222222:instance-profile/foreign</Arn>{id}<Roles><member><RoleId>AROA00000000000000000</RoleId></member></Roles></InstanceProfile>"))).await;
        let p = profile(v);
        assert!(matches!(p.arn, IdentityMemberV3::Present(_)));
        let ObservationValueV2::Present(roles) = p.roles else {
            panic!()
        };
        assert!(matches!(
            roles.as_slice()[0].id,
            IdentityMemberV3::Present(_)
        ));
        assert_eq!(roles.as_slice()[0].arn, IdentityMemberV3::NotReturned);
    }
}
#[tokio::test]
async fn iam_mixed_empty_malformed_duplicate_and_contradictory_occurrences() {
    let role =
        "<member><Arn>arn:aws:iam::222222222222:role/foreign</Arn><RoleId>bad</RoleId></member>";
    let (_, v) = iam(&iam_xml(&format!("<InstanceProfile><Roles>{role}<member/><member><Arn/><RoleId/></member><member><Arn>bad</Arn><RoleId>AROA00000000000000000</RoleId></member>{role}</Roles></InstanceProfile>"))).await;
    assert_eq!(v.occurrences, 6);
    let p = profile(v);
    let ObservationValueV2::Present(roles) = p.roles else {
        panic!()
    };
    let roles = roles.as_slice();
    assert_eq!(roles.len(), 5);
    assert_eq!(roles[0], roles[4]);
    assert!(matches!(roles[0].arn, IdentityMemberV3::Present(_)));
    assert!(matches!(roles[0].id, IdentityMemberV3::Malformed(_)));
    assert_eq!(roles[1].arn, IdentityMemberV3::NotReturned);
    assert_eq!(roles[1].id, IdentityMemberV3::NotReturned);
    assert_eq!(roles[2].arn, IdentityMemberV3::Empty);
    assert_eq!(roles[2].id, IdentityMemberV3::Empty);
    assert!(matches!(roles[3].arn, IdentityMemberV3::Malformed(_)));
    assert!(matches!(roles[3].id, IdentityMemberV3::Present(_)));
}
#[tokio::test]
async fn independent_invocations_cannot_cross_pair_presence_or_values() {
    let first = iam_xml("<InstanceProfile><Arn>first-invalid</Arn></InstanceProfile>");
    let second = iam_xml(
        "<InstanceProfile><InstanceProfileId>AIPA00000000000000000</InstanceProfileId></InstanceProfile>",
    );
    let (a, b) = tokio::join!(iam(&first), iam(&second));
    let a = profile(a.1);
    let b = profile(b.1);
    assert!(matches!(a.arn, IdentityMemberV3::Malformed(_)));
    assert_eq!(a.id, IdentityMemberV3::NotReturned);
    assert_eq!(b.arn, IdentityMemberV3::NotReturned);
    assert!(matches!(b.id, IdentityMemberV3::Present(_)));
}
#[tokio::test]
async fn sdk_decodes_escaped_values_and_presence_ignores_unrelated_names() {
    let (_, v) = iam("<i:GetInstanceProfileResponse xmlns:i=\"https://iam.amazonaws.com/doc/2010-05-08/\"><i:GetInstanceProfileResult><i:InstanceProfile><Other><Arn>ignored</Arn><Roles><member/></Roles></Other><i:Arn>bad&amp;value</i:Arn><i:Roles><i:member><i:RoleId> </i:RoleId></i:member></i:Roles></i:InstanceProfile></i:GetInstanceProfileResult></i:GetInstanceProfileResponse>").await;
    let p = profile(v);
    assert_eq!(
        p.arn,
        IdentityMemberV3::Malformed("bad&value".to_owned().try_into().unwrap())
    );
    let ObservationValueV2::Present(roles) = p.roles else {
        panic!()
    };
    assert_eq!(roles.as_slice().len(), 1);
    assert!(matches!(
        roles.as_slice()[0].id,
        IdentityMemberV3::Malformed(_)
    ));
}
