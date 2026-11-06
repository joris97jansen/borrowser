use net::{HeaderError, HttpHeaders};

#[test]
fn ai2_fields_preserve_order_repetitions_and_bytes() {
    let mut h = HttpHeaders::new();
    h.append("AcCePt", b"one").unwrap();
    h.append("X-Private", &[0x80, 0xff, b'\t']).unwrap();
    h.append("ACCEPT", b"two").unwrap();
    assert_eq!(
        h.iter().collect::<Vec<_>>(),
        vec![
            ("accept", b"one".as_slice()),
            ("x-private", [0x80, 0xff, b'\t'].as_slice()),
            ("accept", b"two".as_slice()),
        ]
    );
    assert_eq!(
        format!("{h:?}"),
        "[(\"accept\", \"<redacted>\"), (\"<redacted>\", \"<redacted>\"), (\"accept\", \"<redacted>\")]"
    );
}

#[test]
fn ai2_invalid_fields_leave_the_list_unchanged() {
    let mut h = HttpHeaders::new();
    h.append("accept", b"unchanged").unwrap();
    for name in ["", "bad name", "bad:name", "bad\rname", "é"] {
        assert_eq!(h.append(name, b"private"), Err(HeaderError::InvalidName));
    }
    for value in [b"a\r\nb".as_slice(), b"a\0b", b"a\x7fb", b"a\x01b"] {
        assert_eq!(h.append("accept", value), Err(HeaderError::InvalidValue));
    }
    assert_eq!(
        h.iter().collect::<Vec<_>>(),
        [("accept", b"unchanged".as_slice())]
    );
    let error = h.append("private:secret", b"secret").unwrap_err();
    assert_eq!(
        format!("{error}"),
        "HTTP field construction failed: InvalidName"
    );
}

#[test]
fn ai2_field_bounds_include_exact_limits_and_atomic_failure() {
    let mut h = HttpHeaders::new();
    assert_eq!(
        h.append(&"a".repeat(257), b""),
        Err(HeaderError::NameTooLong)
    );
    assert_eq!(
        h.append("a", &vec![b'a'; 8193]),
        Err(HeaderError::ValueTooLong)
    );
    h.append(&"a".repeat(256), &vec![b'a'; 8192]).unwrap();
    let mut count = HttpHeaders::new();
    for _ in 0..64 {
        count.append("a", b"").unwrap();
    }
    assert_eq!(count.append("a", b""), Err(HeaderError::TooManyFields));
    let mut total = HttpHeaders::new();
    for _ in 0..7 {
        total.append("a", &vec![b'x'; 8192]).unwrap();
    }
    total.append("a", &vec![b'x'; 8152]).unwrap(); // exactly 65536 incl. overhead
    assert_eq!(total.append("a", b""), Err(HeaderError::ListTooLarge));
    assert_eq!(total.iter().len(), 8);
}

#[test]
fn ai2_nested_field_debug_is_bounded_and_redacts_arbitrary_names() {
    let mut h = HttpHeaders::new();
    for _ in 0..64 {
        h.append("SECRET_NAME", &[b'S'; 100]).unwrap();
    }
    let output = format!("{:#?}", Ok::<_, HeaderError>(Some(h)));
    assert!(!output.contains("SECRET"));
    assert!(!output.contains("secret_name"));
    assert!(!output.contains("SSSS"));
    assert!(output.len() < 10000);
}
