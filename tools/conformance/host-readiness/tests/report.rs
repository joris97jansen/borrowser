//! Report parsing only: these synthetic observations are never Linux evidence.
use borrowser_host_readiness::report::*;
fn fixture(probe: HostProbe) -> Report {
    let mut r = Report::new(probe);
    r.add("alpha", "synthetic\tobservation\n");
    r.add("beta", "not native evidence");
    r
}
#[test]
fn canonical_report_binds_the_exact_requested_probe() {
    for probe in HostProbe::ALL {
        let r = fixture(probe);
        let bytes = r.canonical().unwrap();
        assert_eq!(
            Report::parse_expected(&bytes, probe)
                .unwrap()
                .canonical()
                .unwrap(),
            bytes
        );
        for other in HostProbe::ALL {
            if probe != other {
                assert!(Report::parse_expected(&bytes, other).is_err());
            }
        }
    }
}
#[test]
fn rejects_wrong_format_authority_and_probe() {
    let p = HostProbe::Seccomp;
    let mut r = fixture(p);
    r.format = "other-format".into();
    assert!(r.canonical().is_err());
    let mut bytes = serde_json::to_vec(&r).unwrap();
    bytes.push(b'\n');
    assert!(Report::parse_expected(&bytes, p).is_err());
    let mut r = fixture(p);
    r.authority = "mechanism-go".into();
    assert!(r.canonical().is_err());
    let mut bytes = serde_json::to_vec(&r).unwrap();
    bytes.push(b'\n');
    assert!(Report::parse_expected(&bytes, p).is_err());
    let raw = String::from_utf8(fixture(p).canonical().unwrap()).unwrap();
    assert!(
        Report::parse_expected(raw.replace(p.command(), "unknown-host-probe").as_bytes(), p)
            .is_err()
    );
    assert!(HostProbe::parse("unknown-host-probe").is_err());
}
#[test]
fn rejects_duplicate_names_and_noncanonical_order_or_bytes() {
    let p = HostProbe::UnshareFd;
    let mut r = fixture(p);
    r.observations[1].name = "alpha".into();
    assert!(r.bytes().is_err());
    let mut r = fixture(p);
    r.observations.swap(0, 1);
    let mut unsorted = serde_json::to_vec(&r).unwrap();
    unsorted.push(b'\n');
    assert!(Report::parse_expected(&unsorted, p).is_err());
    assert!(r.canonical().is_err());
    assert!(Report::parse_expected(&r.bytes().unwrap(), p).is_ok());
    let bytes = fixture(p).canonical().unwrap();
    for bad in [
        bytes[..bytes.len() - 1].to_vec(),
        [b" ".as_slice(), &bytes].concat(),
        [b"\xef\xbb\xbf".as_slice(), &bytes].concat(),
    ] {
        assert!(Report::parse_expected(&bad, p).is_err());
    }
    let raw = String::from_utf8(bytes).unwrap();
    for raw in [
        raw.replacen('{', "{\"format\":\"duplicate\",", 1),
        raw.replacen('{', "{\"unknown\":0,", 1),
    ] {
        assert!(Report::parse_expected(raw.as_bytes(), p).is_err());
    }
}
#[test]
fn report_bounds_and_empty_observations_are_enforced() {
    let p = HostProbe::ProcessInspection;
    assert!(Report::new(p).canonical().is_err());
    let mut r = fixture(p);
    r.observations = vec![r.observations[0].clone(); OBSERVATIONS + 1];
    assert!(r.bytes().is_err());
    let mut r = fixture(p);
    r.observations[0].value = "a".repeat(VALUE_BYTES + 1);
    assert!(r.canonical().is_err());
    let mut r = fixture(p);
    r.observations[0].name = "a".repeat(129);
    assert!(r.bytes().is_err());
    let mut r = fixture(p);
    r.observations.clear();
    for i in 0..OBSERVATIONS {
        r.add(&format!("n-{i:02}"), "a".repeat(VALUE_BYTES));
    }
    assert!(r.canonical().is_err());
    assert!(Report::parse_expected(&vec![b' '; REPORT_BYTES + 1], p).is_err());
}
