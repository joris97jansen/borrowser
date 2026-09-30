use super::*;
use crate::aws::{
    configuration,
    identity_observation_tests::iam_xml,
    response_limits::BoundedHttp,
    tests::{replay, response, secret},
};
use aws_smithy_runtime_api::client::http::SharedHttpClient;

async fn assert_attribute_rejected(body: &str) {
    let round = ObservationRound::test();
    let transport = replay(vec![response(200, body, None)]);
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(SharedHttpClient::new(transport.clone()), round.clone()),
    )
    .unwrap();
    let client = aws_sdk_iam::Client::from_conf(aws_sdk_iam::config::Builder::from(&conf).build());
    let (hook, receiver) = capture_instance_profile(round);
    let operation = client
        .get_instance_profile()
        .instance_profile_name("reviewed")
        .customize()
        .interceptor(hook)
        .send()
        .await;
    let captured = receiver.take();
    assert_eq!(
        (operation.is_err(), captured.is_err()),
        (true, true),
        "unsafe attribute must reject both the SDK operation and capture"
    );
    assert_eq!(transport.actual_requests().count(), 1, "no SDK retry");
}

#[tokio::test]
async fn attribute_unescape_counterexample_rejects_operation_and_capture() {
    assert_attribute_rejected(&iam_xml(
        "<InstanceProfile>\n  <Arn xmlns:x=\"&bogus;\">arn:aws:iam::222222222222:instance-profile/foreign</Arn>\n  <InstanceProfileId>AIPA00000000000000000</InstanceProfileId>\n</InstanceProfile>",
    ))
    .await;
}

#[tokio::test]
async fn attribute_references_reject_on_all_structural_paths() {
    // Cross product: all monitored/ignored locations and invalid/valid references.
    // Plain attributes on ignored elements also exercise the rule outside xmlns.
    let locations = [
        "<InstanceProfile><Arn xmlns:x=\"VALUE\">arn:aws:iam::222222222222:instance-profile/foreign</Arn><InstanceProfileId>AIPA00000000000000000</InstanceProfileId></InstanceProfile>",
        "<InstanceProfile><InstanceProfileId xmlns:x=\"VALUE\">AIPA00000000000000000</InstanceProfileId><Arn>arn:aws:iam::222222222222:instance-profile/foreign</Arn></InstanceProfile>",
        "<InstanceProfile><Roles><member><Arn xmlns:x=\"VALUE\">arn:aws:iam::222222222222:role/foreign</Arn><RoleId>AROA00000000000000000</RoleId></member></Roles></InstanceProfile>",
        "<InstanceProfile><Roles><member><RoleId xmlns:x=\"VALUE\">AROA00000000000000000</RoleId><Arn>arn:aws:iam::222222222222:role/foreign</Arn></member></Roles></InstanceProfile>",
        "<InstanceProfile><Ignored arbitrary=\"VALUE\"/><Arn>arn:aws:iam::222222222222:instance-profile/foreign</Arn></InstanceProfile>",
        "<InstanceProfile><Ignored><Nested arbitrary=\"VALUE\"/></Ignored><InstanceProfileId>AIPA00000000000000000</InstanceProfileId></InstanceProfile>",
        "<InstanceProfile xmlns:x=\"VALUE\"><Arn/></InstanceProfile>",
        "<InstanceProfile><Roles xmlns:x=\"VALUE\"><member/></Roles></InstanceProfile>",
        "<InstanceProfile><Roles><member xmlns:x=\"VALUE\"><Arn/></member></Roles></InstanceProfile>",
    ];
    let values = [
        "&bogus;",
        "unterminated&",
        "&#xZZ;",
        "&#no;",
        "&#x110000;",
        "&amp;",
        "&lt;",
        "&#38;",
        "&#x41;",
    ];
    for value in values {
        for location in locations {
            assert_attribute_rejected(&iam_xml(&location.replace("VALUE", value))).await;
        }
        for wrapper in ["GetInstanceProfileResponse", "GetInstanceProfileResult"] {
            let body = iam_xml("<InstanceProfile><Arn/></InstanceProfile>").replacen(
                wrapper,
                &format!("{wrapper} xmlns:x=\"{value}\""),
                1,
            );
            assert_attribute_rejected(&body).await;
        }
    }
}

#[tokio::test]
async fn plain_attributes_preserve_sdk_values_and_role_occurrences() {
    use crate::aws::identity_observation_tests::iam;
    use crate::provider::identity_observation_v3::IdentityMemberV3;
    let role = "<member xmlns:r=\"urn:plain-role\"><Arn>arn:aws:iam::222222222222:role/foreign</Arn><Ignored arbitrary=\"plain\"/></member>";
    let body = format!(
        "<GetInstanceProfileResponse xmlns=\"https://iam.amazonaws.com/doc/2010-05-08/\"><GetInstanceProfileResult xmlns:x=\"urn:plain-result\"><InstanceProfile xmlns:p=\"urn:plain-profile\"><Ignored arbitrary=\"plain\"><Nested another=\"\"/></Ignored><Arn xmlns:a=\"urn:plain-arn\">bad&amp;value</Arn><InstanceProfileId/><Roles xmlns:l=\"urn:plain-list\">{role}<member><RoleId>AROA00000000000000000</RoleId></member>{role}<member/></Roles></InstanceProfile></GetInstanceProfileResult></GetInstanceProfileResponse>"
    );
    let (sdk, observation) = iam(&body).await;
    assert_eq!(sdk.instance_profile().unwrap().arn(), "bad&value");
    assert_eq!(observation.occurrences, 5);
    let ObservationDataV3::Profile {
        profile: ObservationValueV2::Present(profile),
    } = observation.data
    else {
        panic!()
    };
    assert_eq!(
        profile.arn,
        IdentityMemberV3::Malformed("bad&value".to_owned().try_into().unwrap())
    );
    assert_eq!(profile.id, IdentityMemberV3::Empty);
    let ObservationValueV2::Present(roles) = profile.roles else {
        panic!()
    };
    let roles = roles.as_slice();
    assert_eq!(roles.len(), 4);
    assert_eq!(roles[0], roles[2]);
    assert_eq!(
        roles[0].arn,
        IdentityMemberV3::Present("arn:aws:iam::222222222222:role/foreign".parse().unwrap())
    );
    assert_eq!(roles[0].id, IdentityMemberV3::NotReturned);
    assert_eq!(roles[1].arn, IdentityMemberV3::NotReturned);
    assert_eq!(
        roles[1].id,
        IdentityMemberV3::Present("AROA00000000000000000".parse().unwrap())
    );
    assert_eq!(roles[3].arn, IdentityMemberV3::NotReturned);
    assert_eq!(roles[3].id, IdentityMemberV3::NotReturned);
}

#[test]
fn fixed_path_presence_bounds_and_ambiguous_xml() {
    let round = ObservationRound::test();
    let p = scan(
        iam_xml(
            "<InstanceProfile><Roles><member/><member><Arn/></member></Roles></InstanceProfile>",
        )
        .as_bytes(),
        &round,
    )
    .unwrap();
    assert_eq!(p.roles.len(), 2);
    assert!(!p.roles[0].arn);
    assert!(p.roles[1].arn);
    for profile in [
        "<InstanceProfile><Arn/><Arn/></InstanceProfile>",
        "<InstanceProfile><Roles>not-a-list</Roles></InstanceProfile>",
        "<InstanceProfile><Roles><![CDATA[not-a-list]]></Roles></InstanceProfile>",
        "<InstanceProfile/><InstanceProfile/>",
        "<InstanceProfile><Roles/><Roles/></InstanceProfile>",
        "<InstanceProfile><Arn><Nested/></Arn></InstanceProfile>",
        "<InstanceProfile><Arn>a<!--split-->b</Arn></InstanceProfile>",
        "<InstanceProfile><Arn><![CDATA[value]]></Arn></InstanceProfile>",
        "<InstanceProfile><Arn nil=\"true\"/></InstanceProfile>",
        "<InstanceProfile><Roles><member><RoleId/><RoleId/></member></Roles></InstanceProfile>",
    ] {
        assert!(
            scan(iam_xml(profile).as_bytes(), &round).is_err(),
            "{profile}"
        );
    }
    for body in [
        "<GetInstanceProfileResponse><GetInstanceProfileResult></GetInstanceProfileResponse></GetInstanceProfileResult>",
        "<!DOCTYPE root [<!ENTITY a 'v'>]><GetInstanceProfileResponse><GetInstanceProfileResult/></GetInstanceProfileResponse>",
        "<GetInstanceProfileResponse><GetInstanceProfileResult/><GetInstanceProfileResult/></GetInstanceProfileResponse>",
        "<GetInstanceProfileResponse><Metadata/><GetInstanceProfileResult/></GetInstanceProfileResponse>",
    ] {
        assert!(scan(body.as_bytes(), &round).is_err());
    }
    let whitespace = iam_xml("<InstanceProfile> \n<Roles> \t\r\n</Roles></InstanceProfile>");
    assert!(scan(whitespace.as_bytes(), &round).unwrap().roles_present);
    let depth = iam_xml(&format!("{}{}", "<x>".repeat(126), "</x>".repeat(126)));
    assert!(scan(depth.as_bytes(), &round).is_ok());
    for count in [128, 129] {
        let attrs = (0..count)
            .map(|i| format!(" a{i}=\"v\""))
            .collect::<String>();
        let body = iam_xml(&format!("<Other{attrs}/><InstanceProfile/>"));
        assert_eq!(scan(body.as_bytes(), &round).is_ok(), count == 128);
    }
    let mut exact = iam_xml("<InstanceProfile/>");
    exact.push_str(&" ".repeat(RESPONSE_BYTES as usize - exact.len()));
    assert!(scan(exact.as_bytes(), &round).is_ok());
    assert!(scan(&[255], &round).is_err());
    assert!(scan(&vec![b' '; RESPONSE_BYTES as usize + 1], &round).is_err());
    let body = iam_xml(&format!(
        "<InstanceProfile><Roles>{}</Roles></InstanceProfile>",
        "<member/>".repeat(128)
    ));
    assert_eq!(scan(body.as_bytes(), &round).unwrap().roles.len(), 128);
    let body = iam_xml(&format!(
        "<InstanceProfile><Roles>{}</Roles></InstanceProfile>",
        "<member/>".repeat(129)
    ));
    assert!(scan(body.as_bytes(), &round).is_err());
    let body = iam_xml(&format!("{}{}", "<x>".repeat(127), "</x>".repeat(127)));
    assert!(scan(body.as_bytes(), &round).is_err());
}
#[tokio::test]
async fn capture_errors_cannot_produce_successful_observations() {
    for (status, body) in [
        (
            403,
            "<ErrorResponse><Error><Code>AccessDenied</Code></Error></ErrorResponse>".to_owned(),
        ),
        (
            200,
            iam_xml("<InstanceProfile><Arn/><Arn/></InstanceProfile>"),
        ),
        (
            200,
            iam_xml("<InstanceProfile><CreateDate>bad-date</CreateDate><Arn/></InstanceProfile>"),
        ),
        (200, " ".repeat(RESPONSE_BYTES as usize + 1)),
    ] {
        let round = ObservationRound::test();
        let conf = configuration(
            &secret(),
            &"eu-central-1".parse().unwrap(),
            BoundedHttp::new(
                SharedHttpClient::new(replay(vec![response(status, &body, None)])),
                round.clone(),
            ),
        )
        .unwrap();
        let client =
            aws_sdk_iam::Client::from_conf(aws_sdk_iam::config::Builder::from(&conf).build());
        let (hook, receiver) = capture_instance_profile(round);
        assert!(
            client
                .get_instance_profile()
                .instance_profile_name("reviewed")
                .customize()
                .interceptor(hook)
                .send()
                .await
                .is_err()
        );
        assert!(receiver.take().is_err());
    }
}
#[test]
fn correlation_mismatch_and_incomplete_lifecycle_reject() {
    let round = ObservationRound::test();
    let (hook, receiver) = capture_instance_profile(round.clone());
    assert!(receiver.take().is_err());
    assert!(
        hook.transition(|s| {
            require(matches!(s, State::Fresh), "state")?;
            Ok(State::Started)
        })
        .is_err()
    );
    let (hook, receiver) = capture_instance_profile(round.clone());
    hook.transition(|_| Ok(State::Started)).unwrap();
    assert!(receiver.take().is_err());
    let (hook, receiver) = capture_instance_profile(round);
    hook.transition(|_| Ok(State::Started)).unwrap();
    assert!(
        hook.transition(|s| {
            require(matches!(s, State::Fresh), "repeated invocation")?;
            Ok(State::Started)
        })
        .is_err()
    );
    assert!(receiver.take().is_err());
}
#[tokio::test]
async fn different_operation_cannot_use_capture() {
    let round = ObservationRound::test();
    let replay = replay(vec![]);
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(SharedHttpClient::new(replay), round.clone()),
    )
    .unwrap();
    let client = aws_sdk_iam::Client::from_conf(aws_sdk_iam::config::Builder::from(&conf).build());
    let (hook, receiver) = capture_instance_profile(round);
    assert!(
        client
            .get_role()
            .role_name("synthetic")
            .customize()
            .interceptor(hook)
            .send()
            .await
            .is_err()
    );
    assert!(receiver.take().is_err());
}

#[tokio::test]
async fn correlation_cardinality_and_corrected_defaults_are_verified() {
    use crate::aws::identity_observation_tests::iam;
    let body = iam_xml(
        "<InstanceProfile><Arn>returned</Arn><Roles><member><RoleId>returned</RoleId></member></Roles></InstanceProfile>",
    );
    let (output, _) = iam(&body).await;
    let round = ObservationRound::test();
    let other = scan(
        iam_xml("<InstanceProfile><Roles/></InstanceProfile>").as_bytes(),
        &round,
    )
    .unwrap();
    assert!(normalize(&output, other).is_err());
    let other = scan(
        iam_xml("<InstanceProfile><Arn/><Roles><member/></Roles></InstanceProfile>").as_bytes(),
        &round,
    )
    .unwrap();
    assert!(normalize(&output, other).is_err());
    assert!(normalize(&output, ProfilePresence::default()).is_err());
}

#[tokio::test]
async fn session_expiry_after_capture_prevents_receiving_success() {
    let round = ObservationRound::test();
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(
            SharedHttpClient::new(replay(vec![response(
                200,
                &iam_xml("<InstanceProfile/>"),
                None,
            )])),
            round.clone(),
        ),
    )
    .unwrap();
    let client = aws_sdk_iam::Client::from_conf(aws_sdk_iam::config::Builder::from(&conf).build());
    let (hook, receiver) = capture_instance_profile(round.clone());
    client
        .get_instance_profile()
        .instance_profile_name("reviewed")
        .customize()
        .interceptor(hook)
        .send()
        .await
        .unwrap();
    assert!(round.bind_expiration(std::time::UNIX_EPOCH).is_err());
    assert!(receiver.take().is_err());
    assert!(scan(iam_xml("<InstanceProfile/>").as_bytes(), &round).is_err());
}

#[tokio::test]
async fn dropped_sdk_future_cannot_complete_capture_or_reuse_round() {
    use std::{
        pin::Pin,
        sync::atomic::{AtomicBool, Ordering},
        task::{Context, Poll},
    };
    struct PendingBody(Arc<AtomicBool>, Arc<AtomicBool>);
    impl http_body::Body for PendingBody {
        type Data = bytes::Bytes;
        type Error = std::io::Error;
        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<std::result::Result<http_body::Frame<Self::Data>, Self::Error>>> {
            self.0.store(true, Ordering::SeqCst);
            Poll::Pending
        }
    }
    impl Drop for PendingBody {
        fn drop(&mut self) {
            self.1.store(true, Ordering::SeqCst);
        }
    }
    let polled = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let body = aws_smithy_types::body::SdkBody::from_body_1_x(PendingBody(
        polled.clone(),
        dropped.clone(),
    ));
    let response = http::Response::builder().status(200).body(body).unwrap();
    let round = ObservationRound::test();
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(SharedHttpClient::new(replay(vec![response])), round.clone()),
    )
    .unwrap();
    let client = aws_sdk_iam::Client::from_conf(aws_sdk_iam::config::Builder::from(&conf).build());
    let (hook, receiver) = capture_instance_profile(round.clone());
    let mut future = Box::pin(
        client
            .get_instance_profile()
            .instance_profile_name("reviewed")
            .customize()
            .interceptor(hook)
            .send(),
    );
    tokio::time::timeout(std::time::Duration::from_secs(2),async {
        tokio::select! { result=&mut future=>panic!("unexpected completion: {}",result.is_ok()), _=async {while !polled.load(Ordering::SeqCst) {tokio::task::yield_now().await;}}=>() }
    }).await.unwrap();
    drop(future);
    assert!(dropped.load(Ordering::SeqCst));
    assert!(receiver.take().is_err());
    assert!(round.remaining().is_err());
}
