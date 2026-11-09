//! AI5 Step B: field admission and authoritative framing selection through the
//! pinned production parser. No public network or replacement parser is used.
use std::io::Read;

fn response(status: u16, fields: &str, body: &str) -> Result<ureq::Response, Box<ureq::Error>> {
    format!("HTTP/1.1 {status} Response\r\n{fields}\r\n{body}")
        .parse()
        .map_err(Box::new)
}

#[test]
fn controls_obs_fold_and_non_token_names_are_rejected_without_echo() {
    for field in [
        "Bad Name: PRIVATE",
        "X: PRIVATE\0",
        "X: PRIVATE\u{000b}",
        "X: PRIVATE\u{000c}",
        "X: PRIVATE\u{001f}",
        "X: PRIVATE\u{007f}",
        "X: PRIVATE\rembedded",
        " X: PRIVATE",
        "\tPRIVATE",
        "X: ok\nPRIVATE",
    ] {
        let error = response(200, &format!("{field}\r\n"), "").unwrap_err();
        assert_eq!(error.kind(), ureq::ErrorKind::BadHeader);
        for diagnostic in [
            format!("{error}"),
            format!("{error:?}"),
            format!("{error:#?}"),
        ] {
            assert!(!diagnostic.contains("PRIVATE"));
        }
    }
}

#[test]
fn token_names_obs_text_and_only_boundary_ows_are_preserved() {
    let response = response(200,
        "!#$%&'*+-.^_`|~09AZaz: valid\r\nX-A: \t\u{00a0}café\u{00a0} \t\r\nX-B: middle\r\nX-A:\t\r\n", "").unwrap();
    assert_eq!(
        response.headers_bytes().collect::<Vec<_>>(),
        [
            ("!#$%&'*+-.^_`|~09AZaz", b"valid".as_slice()),
            ("X-A", "\u{00a0}café\u{00a0}".as_bytes()),
            ("X-B", b"middle".as_slice()),
            ("X-A", b"".as_slice()),
        ]
    );
    assert_eq!(response.all("x-a"), ["\u{00a0}café\u{00a0}", ""]);
}

#[test]
fn content_length_requires_identical_unsigned_decimal_members() {
    for values in [
        "",
        " ",
        "-1",
        "+1",
        "1x",
        "1 1",
        "1,",
        ",1",
        "1,,1",
        "1,2",
        "1,\u{00a0}1",
    ] {
        assert_eq!(
            response(200, &format!("Content-Length: {values}\r\n"), "a")
                .unwrap_err()
                .kind(),
            ureq::ErrorKind::BadHeader
        );
    }
    for (fields, expected_values) in [
        (
            "Content-Length: 003\r\nContent-Length: 3, 03\r\n",
            vec!["003", "3, 03"],
        ),
        ("Content-Length: \t3 ,\t3 \t\r\n", vec!["3 ,\t3"]),
    ] {
        let response = response(200, fields, "abcEXTRA").unwrap();
        let values = response.all("content-length");
        assert_eq!(
            values, expected_values,
            "admitted occurrences must not be rewritten"
        );
        drop(values);
        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body).unwrap();
        assert_eq!(body, b"abc");
    }
}

#[test]
fn largest_u64_length_is_metadata_not_an_allocation_request() {
    let acquired = response(200, &format!("Content-Length: {}\r\n", u64::MAX), "abc").unwrap();
    let mut body = Vec::new();
    assert_eq!(
        acquired
            .into_reader()
            .read_to_end(&mut body)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::UnexpectedEof
    );
    assert_eq!(body, b"abc");
    let overflow = u128::from(u64::MAX) + 1;
    assert_eq!(
        response(200, &format!("Content-Length: {overflow}\r\n"), "")
            .unwrap_err()
            .kind(),
        ureq::ErrorKind::BadHeader
    );
}

#[test]
fn u64_lengths_above_u32_preserve_equality_and_incomplete_body_behavior() {
    for length in [u64::from(u32::MAX), u64::from(u32::MAX) + 1, u64::MAX] {
        let fields = format!("Content-Length: {length}\r\nContent-Length: {length}, 0{length}\r\n");
        let mut body = Vec::new();
        let error = response(200, &fields, "abc")
            .unwrap()
            .into_reader()
            .read_to_end(&mut body)
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(body, b"abc");
        let metadata = response(304, &fields, "").unwrap();
        assert_eq!(
            metadata.all("content-length"),
            [length.to_string(), format!("{length}, 0{length}")]
        );
        let mut empty = Vec::new();
        metadata.into_reader().read_to_end(&mut empty).unwrap();
        assert!(empty.is_empty());
        assert_eq!(
            response(
                200,
                &format!(
                    "Content-Length: {length}\r\nContent-Length: {}\r\n",
                    length - 1
                ),
                ""
            )
            .unwrap_err()
            .kind(),
            ureq::ErrorKind::BadHeader
        );
    }
}

#[test]
fn transfer_syntax_is_validated_before_supported_subset_selection() {
    for coding in [
        "",
        ", ,",
        "chunked gzip",
        "chunked, @",
        "chunked, chunked",
        "chunked,,chunked",
        "chunked, gzip, chunked",
        "chunked;foo=bar",
        "gzip;foo=bar",
        "custom;foo",
        "custom;=x",
        "custom;foo=",
        "custom;foo=\"unterminated",
        "custom;foo=\"x\"junk",
    ] {
        for status in [200, 304] {
            assert_eq!(
                response(
                    status,
                    &format!("Transfer-Encoding: {coding}\r\n"),
                    "0\r\n\r\n"
                )
                .unwrap_err()
                .kind(),
                ureq::ErrorKind::BadHeader,
                "{coding}"
            );
        }
    }
    for coding in [
        "gzip",
        "gzip, chunked",
        "chunked,,gzip",
        // RFC 9112 section 6.3 permits response framing by connection closure
        // when chunked is non-final. AI5 does not execute this coding stack.
        "chunked, gzip",
        "chunked, custom;foo=\"a,b\"",
        "custom;foo=\"a,b\\\"c\", chunked",
    ] {
        assert_eq!(
            response(200, &format!("Transfer-Encoding: {coding}\r\n"), "")
                .unwrap_err()
                .kind(),
            ureq::ErrorKind::UnsupportedResponse
        );
    }
    let mut body = Vec::new();
    response(
        200,
        "Transfer-Encoding: \tChUnKeD \t\r\n",
        "3\r\nabc\r\n0\r\n\r\n",
    )
    .unwrap()
    .into_reader()
    .read_to_end(&mut body)
    .unwrap();
    assert_eq!(body, b"abc");
}

#[test]
fn transfer_recipient_list_ignores_empty_members_without_rewriting_fields() {
    // RFC 9110 section 5.6.1.2 requires recipient tolerance even though senders
    // must not generate empty members. Count actual codings across occurrences.
    for values in [
        vec!["chunked"],
        vec![",chunked"],
        vec!["chunked,"],
        vec![", ,chunked,,"],
        vec!["", ", ,", ",ChUnKeD,", ",", ""],
        vec!["chunked,", "", ", ,"],
    ] {
        let fields: String = values
            .iter()
            .map(|value| format!("Transfer-Encoding: {value}\r\n"))
            .collect();
        let acquired = response(200, &fields, "3\r\nabc\r\n0\r\n\r\n").unwrap();
        assert_eq!(
            acquired.headers_bytes().collect::<Vec<_>>(),
            values
                .iter()
                .map(|value| ("Transfer-Encoding", value.as_bytes()))
                .collect::<Vec<_>>()
        );
        let mut body = Vec::new();
        acquired.into_reader().read_to_end(&mut body).unwrap();
        assert_eq!(body, b"abc");
    }
}

#[test]
fn transfer_recipient_list_classifies_actual_members_across_fields() {
    use ureq::ErrorKind::{BadHeader, UnsupportedResponse};
    for (values, expected) in [
        (vec!["chunked,,gzip"], UnsupportedResponse),
        (vec!["chunked,", "", ",gzip"], UnsupportedResponse),
        (vec!["gzip,,", "", ",chunked,"], UnsupportedResponse),
        // Quoted commas and apparent coding names remain parameter data.
        (
            vec!["", r#",custom;p=",chunked,,chunked",,chunked,"#],
            UnsupportedResponse,
        ),
        (vec![r#"custom;p="a,b\"c",,chunked,"#], UnsupportedResponse),
        (vec!["chunked,,chunked"], BadHeader),
        (vec![",chunked,", "", ",CHUNKED,"], BadHeader),
        (vec![""], BadHeader),
        (vec![", ,"], BadHeader),
        (vec!["", ",", " \t "], BadHeader),
        (vec!["chunked", ", @"], BadHeader),
        (vec!["", ",chunked gzip"], BadHeader),
        (vec![r#",custom;p="a,b"chunked"#], BadHeader),
        (vec![",custom;p=,chunked"], BadHeader),
        (vec![",custom;p=\"unterminated,chunked"], BadHeader),
        (vec![",chunked;foo=bar,"], BadHeader),
    ] {
        let fields: String = values
            .iter()
            .map(|value| format!("Transfer-Encoding: {value}\r\n"))
            .collect();
        for status in [200, 304] {
            let result = response(status, &fields, "");
            if status == 304 && expected == UnsupportedResponse {
                // A valid representation coding stack does not select a body
                // reader on 304. Preserve the original field occurrences.
                let acquired = result.unwrap();
                assert_eq!(acquired.all("transfer-encoding"), values);
                let mut body = Vec::new();
                acquired.into_reader().read_to_end(&mut body).unwrap();
                assert!(body.is_empty());
            } else {
                assert_eq!(result.unwrap_err().kind(), expected, "{status}: {values:?}");
            }
        }
    }
}

#[test]
fn transfer_empty_member_tolerance_uses_existing_response_head_budgets() {
    let prefix = "Transfer-Encoding: ";
    let suffix = "chunked\r\n\r\n";
    let line_padding = 10 * 1024 - prefix.len() - "chunked\r\n".len();
    let exact_line = format!(
        "HTTP/1.1 200 OK\r\n{prefix}{}{suffix}",
        ",".repeat(line_padding)
    );
    assert_eq!(
        exact_line.split_inclusive('\n').nth(1).unwrap().len(),
        10 * 1024
    );

    let mut exact_head = String::from("HTTP/1.1 200 OK\r\n");
    exact_head.push_str(&format!("{prefix}{}\r\n", ",".repeat(8000)).repeat(7));
    let head_padding = 64 * 1024 - exact_head.len() - prefix.len() - suffix.len();
    exact_head.push_str(&format!("{prefix}{}{suffix}", ",".repeat(head_padding)));
    assert_eq!(exact_head.len(), 64 * 1024);
    assert!(
        exact_head
            .split_inclusive('\n')
            .all(|line| line.len() < 10 * 1024)
    );

    for mut head in [exact_line, exact_head] {
        let acquired = format!("{head}0\r\n\r\n")
            .parse::<ureq::Response>()
            .unwrap();
        let mut body = Vec::new();
        acquired.into_reader().read_to_end(&mut body).unwrap();
        assert!(body.is_empty());
        head.insert(head.len() - suffix.len(), ',');
        assert_eq!(
            format!("{head}0\r\n\r\n")
                .parse::<ureq::Response>()
                .unwrap_err()
                .kind(),
            ureq::ErrorKind::ResponseHeadTooLarge
        );
    }
}

#[test]
fn status_304_accepts_valid_transfer_metadata_without_a_body_reader() {
    for fields in [
        "Transfer-Encoding: gzip, chunked\r\n",
        "Transfer-Encoding: chunked, gzip\r\n",
        "Transfer-Encoding: custom;foo=\"a,b\\\"c\", chunked\r\n",
        "Transfer-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n",
        "Transfer-Encoding: chunked\r\nTransfer-Encoding: gzip\r\n",
    ] {
        let response = response(304, fields, "").unwrap();
        assert!(!response.all("transfer-encoding").is_empty());
        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body).unwrap();
        assert!(body.is_empty());
    }
    assert_eq!(
        response(304, "Content-Length: nope\r\n", "")
            .unwrap_err()
            .kind(),
        ureq::ErrorKind::BadHeader
    );
}

#[test]
fn status_205_retains_chunked_or_close_delimited_reader_for_later_verification() {
    // This is acquisition-layer evidence. Fetch must still reject this payload
    // in Step C; a no-content expectation must not erase the framing reader.
    for (fields, wire_body) in [
        ("Transfer-Encoding: chunked\r\n", "3\r\nabc\r\n0\r\n\r\n"),
        ("", "abc"),
    ] {
        let mut body = Vec::new();
        response(205, fields, wire_body)
            .unwrap()
            .into_reader()
            .read_to_end(&mut body)
            .unwrap();
        assert_eq!(body, b"abc");
    }
}

#[test]
fn invalid_framing_precedes_content_decoder_construction() {
    for fields in [
        "Content-Length: nope\r\n",
        "Content-Length: 3\r\nTransfer-Encoding: chunked\r\n",
    ] {
        let error = response(200, &format!("{fields}Content-Encoding: gzip\r\n"), "").unwrap_err();
        assert_eq!(error.kind(), ureq::ErrorKind::BadHeader);
    }
}

#[test]
fn complete_response_head_has_an_exact_aggregate_byte_budget() {
    let mut head = String::from("HTTP/1.1 200 OK\r\n");
    // Every line is at most 8 KiB: the aggregate boundary must be reached
    // independently of the 10 KiB individual wire-line limit.
    head.push_str(&format!("X: {}\r\n", "a".repeat(8192 - 5)).repeat(7));
    let padding = 64 * 1024 - head.len() - "X: PRIVATE\r\n\r\n".len();
    head.push_str(&format!("X: PRIVATE{}\r\n\r\n", "a".repeat(padding)));
    assert_eq!(head.len(), 64 * 1024);
    assert!(head.split_inclusive('\n').all(|line| line.len() <= 8192));
    assert!(head.parse::<ureq::Response>().is_ok());
    head.insert(head.len() - 4, 'a');
    assert_eq!(head.len(), 64 * 1024 + 1);
    assert!(
        head.split_inclusive('\n')
            .all(|line| line.len() < 10 * 1024)
    );
    let error = head.parse::<ureq::Response>().unwrap_err();
    assert_eq!(error.kind(), ureq::ErrorKind::ResponseHeadTooLarge);
    for diagnostic in [
        format!("{error}"),
        format!("{error:?}"),
        format!("{error:#?}"),
    ] {
        assert!(!diagnostic.contains("PRIVATE"));
    }
    // Many individually small fields must share the same acquisition budget.
    assert_eq!(
        response(200, &format!("X: {}\r\n", "a".repeat(8000)).repeat(9), "")
            .unwrap_err()
            .kind(),
        ureq::ErrorKind::ResponseHeadTooLarge
    );
}

#[test]
fn individual_wire_head_lines_have_an_exact_ten_kib_budget() {
    for (prefix, suffix) in [
        ("HTTP/1.1 200 OK\r\nX: PRIVATE", "\r\n\r\n"),
        ("HTTP/1.1 200 PRIVATE", "\r\n\r\n"),
    ] {
        let line_prefix = prefix
            .rsplit_once("\r\n")
            .map_or(prefix, |(_, field)| field);
        let padding = 10 * 1024 - line_prefix.len() - 2;
        let mut wire = format!("{prefix}{}{suffix}", "a".repeat(padding));
        assert!(
            wire.split_inclusive('\n')
                .any(|line| line.len() == 10 * 1024)
        );
        assert!(wire.len() < 64 * 1024);
        let acquired = wire.parse::<ureq::Response>().unwrap();
        assert_eq!(acquired.status(), 200);
        if prefix.contains("\r\nX:") {
            assert_eq!(
                acquired.header("x").unwrap().len(),
                "PRIVATE".len() + padding
            );
        } else {
            assert_eq!(acquired.status_text().len(), "PRIVATE".len() + padding);
        }
        wire.insert(wire.len() - suffix.len(), 'a');
        assert!(
            wire.split_inclusive('\n')
                .any(|line| line.len() == 10 * 1024 + 1)
        );
        let error = wire.parse::<ureq::Response>().unwrap_err();
        assert_eq!(error.kind(), ureq::ErrorKind::ResponseHeadTooLarge);
        for diagnostic in [
            format!("{error}"),
            format!("{error:?}"),
            format!("{error:#?}"),
        ] {
            assert!(!diagnostic.contains("PRIVATE"));
        }
    }
}

#[test]
fn field_count_is_bounded_without_truncation() {
    assert_eq!(
        response(200, &"X: value\r\n".repeat(100), "")
            .unwrap()
            .headers_bytes()
            .count(),
        100
    );
    assert_eq!(
        response(200, &"X: value\r\n".repeat(101), "")
            .unwrap_err()
            .kind(),
        ureq::ErrorKind::ResponseHeadTooLarge
    );
}

#[test]
fn versions_and_informational_responses_fail_explicitly() {
    for reason in ["PRIVATE\0", "PRIVATE\r", "PRIVATE\u{007f}"] {
        let error = format!("HTTP/1.1 200 {reason}\r\n\r\n")
            .parse::<ureq::Response>()
            .unwrap_err();
        assert_eq!(error.kind(), ureq::ErrorKind::BadStatus);
        assert!(!format!("{error:?}").contains("PRIVATE"));
    }
    for (version, status, kind) in [
        ("HTTP/1.0", "200", None),
        ("HTTP/1.1", "200", None),
        (
            "HTTP/2.0",
            "200",
            Some(ureq::ErrorKind::UnsupportedResponse),
        ),
        (
            "HTTP/1.2",
            "200",
            Some(ureq::ErrorKind::UnsupportedResponse),
        ),
        ("HTTP/1x1", "200", Some(ureq::ErrorKind::BadStatus)),
        ("HTTP/1.1", "+20", Some(ureq::ErrorKind::BadStatus)),
        ("HTTP/1.1", "700", Some(ureq::ErrorKind::BadStatus)),
        (
            "HTTP/1.1",
            "103",
            Some(ureq::ErrorKind::UnsupportedResponse),
        ),
    ] {
        let parsed = format!("{version} {status} Response\r\n\r\n").parse::<ureq::Response>();
        if let Some(kind) = kind {
            assert_eq!(parsed.unwrap_err().kind(), kind);
        } else {
            assert!(parsed.is_ok());
        }
    }
    assert_eq!(
        "HTTP/1.0 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n"
            .parse::<ureq::Response>()
            .unwrap_err()
            .kind(),
        ureq::ErrorKind::BadHeader
    );
    assert_eq!(
        "HTTP/1.1 200 OK\nX: value\n\n"
            .parse::<ureq::Response>()
            .unwrap_err()
            .kind(),
        ureq::ErrorKind::BadHeader
    );
}
