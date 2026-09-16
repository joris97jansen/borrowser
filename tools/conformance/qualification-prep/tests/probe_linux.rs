use qualification_prep::{
    Error,
    error::after_cleanup,
    identity::{self, Framer},
};
use serde_json::json;
#[test]
fn exact_commands_and_fragmented_framing() {
    assert_eq!(
        identity::version_request(),
        b"{\"id\":1,\"method\":\"Browser.getVersion\"}\0"
    );
    assert_eq!(
        identity::close_request(),
        b"{\"id\":2,\"method\":\"Browser.close\"}\0"
    );
    let mut f = Framer::default();
    assert!(f.push(b"{\"id\":1,").unwrap().is_empty());
    let frames = f.push(b"\"result\":{}}\0").unwrap();
    assert_eq!(frames.len(), 1);
    identity::response(&frames[0], 1).unwrap();
    f.finish().unwrap();
}
#[test]
fn preserves_raw_identity_and_revision_semantics() {
    let t = identity::version_result(
        &json!({"product":"HeadlessChrome/123.4","revision":"@abc","protocolVersion":"1.3"}),
    )
    .unwrap();
    assert_eq!(t.product, "HeadlessChrome/123.4");
    assert_eq!(t.revision.as_deref(), Some("@abc"));
    for raw in ["x", "x/y/z", "/v", "x/", " x/v", "x/v\n"] {
        assert!(identity::split_product(raw).is_err());
    }
    for bad in [json!(null), json!(42), json!({}), json!([])] {
        assert!(
            identity::version_result(
                &json!({"product":"x/y","protocolVersion":"p","revision":bad})
            )
            .is_err()
        );
    }
    assert_eq!(
        identity::version_result(&json!({"product":"x/y","protocolVersion":"p"}))
            .unwrap()
            .revision,
        None
    );
    assert_eq!(
        identity::version_result(&json!({"product":"x/y","protocolVersion":"p","revision":""}))
            .unwrap()
            .revision,
        Some(String::new())
    );
}
#[test]
fn rejects_wrong_malformed_truncated_and_oversized_protocol() {
    for v in [
        json!({"id":2,"result":{}}),
        json!({"id":1,"error":{}}),
        json!({"id":1,"result":{},"sessionId":"s"}),
        json!({"method":"event"}),
    ] {
        assert!(identity::response(&v, 1).is_err());
    }
    assert!(Framer::default().push(b"bad\0").is_err());
    assert!(Framer::default().push(b"\0").is_err());
    let mut f = Framer::default();
    f.push(b"{").unwrap();
    assert!(f.finish().is_err());
    assert!(
        Framer::default()
            .push(&vec![b'x'; identity::MESSAGE_BYTES + 1])
            .is_err()
    );
    let mut f = Framer::default();
    let record = [
        b"\"".as_slice(),
        &vec![b'x'; identity::MESSAGE_BYTES - 2],
        b"\"\0",
    ]
    .concat();
    for _ in 0..3 {
        f.push(&record).unwrap();
    }
    assert!(f.push(&record).is_err());
}
#[test]
fn candidate_is_suppressed_by_cleanup_failure() {
    assert_eq!(
        after_cleanup(Ok("candidate"), Err(Error::Cleanup)),
        Err(Error::Cleanup)
    );
    assert_eq!(
        after_cleanup::<()>(Err(Error::Timeout), Err(Error::Invalid("cleanup"))),
        Err(Error::Cleanup)
    );
}
#[cfg(target_os = "linux")]
#[test]
fn namespace_maps_proc_aliases_and_routes() {
    use qualification_prep::probe_linux::{verify_map, verify_proc_mounts, verify_routes};
    verify_map("1000 1000 1\n", 1000).unwrap();
    for s in ["0 1000 1", "1000 0 1", "1000 1000 2", "1000 1000 1\n2 2 1"] {
        assert!(verify_map(s, 1000).is_err());
    }
    verify_proc_mounts("1 0 0:1 / /proc rw,nosuid,nodev,noexec - proc proc rw\n").unwrap();
    assert!(
        verify_proc_mounts(
            "1 0 0:1 / /proc rw - proc proc rw\n2 0 0:1 /sys /alias rw - proc proc rw\n"
        )
        .is_err()
    );
    verify_routes("Iface Destination Gateway Flags\n", "").unwrap();
    assert!(
        verify_routes(
            "Iface Destination Gateway Flags\neth0 00000000 00000000 0001\n",
            ""
        )
        .is_err()
    );
}
#[test]
fn duplicate_protocol_fields_fail() {
    assert!(
        Framer::default()
            .push(b"{\"id\":2,\"id\":1,\"result\":{}}\0")
            .is_err()
    );
}
