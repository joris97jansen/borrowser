use super::*;
fn selected(body: &[u8], label: Option<&str>) -> CssDecodingDecision {
    select_stylesheet_decoding(body, label, None).unwrap()
}
#[test]
fn aj8_encoding_precedence_and_labels() {
    let cases: &[(&[u8], Option<&str>, CssEncodingSource, bool)] = &[
        (b"", None, CssEncodingSource::Default, true),
        (
            b"\xef\xbb\xbf",
            Some("shift_jis"),
            CssEncodingSource::Bom,
            true,
        ),
        (b"\xff\xfe", Some("utf-8"), CssEncodingSource::Bom, false),
        (b"\xfe\xff", None, CssEncodingSource::Bom, false),
        (
            b"@charset \"utf-8\";",
            Some("private"),
            CssEncodingSource::CharsetDeclaration,
            true,
        ),
        (
            b"@charset \"utf-8\";",
            Some("shift_jis"),
            CssEncodingSource::Transport,
            false,
        ),
        (
            b"@charset \"utf-16\";",
            None,
            CssEncodingSource::CharsetDeclaration,
            true,
        ),
        (
            b"@charset \"utf-8\";",
            Some("utf-16"),
            CssEncodingSource::Transport,
            false,
        ),
        (
            b"",
            Some("replacement"),
            CssEncodingSource::Transport,
            false,
        ),
        (b"", Some(" \tUtF8\r\n"), CssEncodingSource::Transport, true),
    ];
    for (body, label, source, supported) in cases {
        let d = selected(body, *label);
        assert_eq!(d.selected_source(), *source);
        assert_eq!(d.effective_encoding().is_utf8(), *supported);
        validate_decision_for_body(body, &d).unwrap();
    }
    for label in ["\u{b}utf-8", "utf-8\u{a0}", "utf-8\0", "ut f-8", ""] {
        assert_eq!(
            resolve_stylesheet_label(Some(label)),
            CssEncodingLabel::Unknown
        );
    }
    eprintln!("AJ8 encoding rows={}", cases.len() + 5);
}
#[test]
fn aj8_exact_charset_prefix_and_1024_boundary() {
    let valid = format!("@charset \"{}utf-8\";", " ".repeat(1007));
    assert_eq!(valid.len(), 1024);
    assert_eq!(
        selected(valid.as_bytes(), None).selected_source(),
        CssEncodingSource::CharsetDeclaration
    );
    let invalid = format!("@charset \"{}utf-8\";", " ".repeat(1008));
    assert_eq!(
        selected(invalid.as_bytes(), None).charset_prefix(),
        CssCharsetPrefix::Malformed
    );
    for input in [
        b" @charset \"utf-8\";".as_slice(),
        b"@CHARSET \"utf-8\";",
        b"@charset  \"utf-8\";",
        b"@charset\t\"utf-8\";",
        b"@charset 'utf-8';",
        b"/*x*/@charset \"utf-8\";",
    ] {
        assert_eq!(
            selected(input, None).charset_prefix(),
            CssCharsetPrefix::NoExactPrefix
        );
    }
    for input in [
        b"@charset \"utf-8\" ;".as_slice(),
        b"@charset \"utf-8",
        b"@charset \"\x80\";",
    ] {
        assert_eq!(
            selected(input, None).charset_prefix(),
            CssCharsetPrefix::Malformed
        );
    }
    for input in [
        b"@charset \"\";".as_slice(),
        b"@charset \"\0\";",
        b"@charset \"\x0b\";",
        b"@charset \"\x7f\";",
    ] {
        assert!(
            matches!(selected(input,None).charset_prefix(),CssCharsetPrefix::Exact(p) if p.label()==CssEncodingLabel::Unknown)
        );
    }
    assert_eq!(
        selected(b"@charset \"shift_jis\";", Some("utf-8")).charset_prefix(),
        CssCharsetPrefix::NotConsulted
    );
    eprintln!("AJ8 charset rows=16");
}
#[test]
fn aj8_environment_uses_actual_aj4_encoding() {
    let doc = stylesheet_environment(CssEncodingLabel::Unknown, Some(CssEncoding::UTF8));
    let d = select_stylesheet_decoding(b"", None, doc).unwrap();
    assert_eq!(d.selected_source(), CssEncodingSource::Environment);
    assert_eq!(
        d.environment().unwrap().source(),
        CssEnvironmentSource::Document
    );
    let link = stylesheet_environment(
        resolve_stylesheet_label(Some("shift_jis")),
        Some(CssEncoding::UTF8),
    );
    assert!(
        !select_stylesheet_decoding(b"", None, link)
            .unwrap()
            .effective_encoding()
            .is_utf8()
    );
    assert_eq!(stylesheet_environment(CssEncodingLabel::Absent, None), None);
}
#[test]
fn aj8_utf8_partition_eof_and_literal_replacement() {
    type ExpectedRecovery<'a> = (&'a [u8], &'a str, usize, usize, u8, u8);
    let cases: &[ExpectedRecovery<'_>] = &[
        (b"", "", 0, 0, 0, 0),
        (b"A", "A", 0, 0, 0, 0),
        (b"\xef\xbf\xbd", "\u{fffd}", 0, 0, 0, 0),
        (b"\xff", "\u{fffd}", 1, 1, 0, 0),
        (b"\xc0\xaf", "\u{fffd}\u{fffd}", 2, 2, 0, 0),
        (b"\xe2\x82\xac", "€", 0, 0, 0, 0),
        (b"\xe2\x82", "\u{fffd}", 0, 0, 1, 2),
        (b"\xe2(\xa1", "\u{fffd}(\u{fffd}", 2, 2, 0, 0),
        (b"\xf0\x90\x80A", "\u{fffd}A", 1, 3, 0, 0),
        (b"\xed\xa0\x80", "\u{fffd}\u{fffd}\u{fffd}", 3, 3, 0, 0),
        (
            b"\xf4\x90\x80\x80",
            "\u{fffd}\u{fffd}\u{fffd}\u{fffd}",
            4,
            4,
            0,
            0,
        ),
        (b"\x80\xe2\x82", "\u{fffd}\u{fffd}", 1, 1, 1, 2),
        (b"\xef\xbb", "\u{fffd}", 0, 0, 1, 2),
        (b"\xef\xbb\xbf\xef\xbb\xbf", "\u{feff}", 0, 0, 0, 0),
    ];
    let mut rows = 0;
    for &(bytes, text, i, ai, f, af) in cases {
        let decision = selected(bytes, None);
        for split in 0..=bytes.len() {
            let decoded =
                decode_with_partitions(bytes, decision, &[split, bytes.len()], None, false)
                    .unwrap();
            assert_eq!(decoded.text().unwrap(), text);
            let CssDecodeReport::Finalized(r) = decoded.report() else {
                panic!("finalized")
            };
            let c = r.replacements();
            assert_eq!(
                (
                    c.invalid_events(),
                    c.invalid_affected_bytes(),
                    c.incomplete_eof_events(),
                    c.incomplete_eof_affected_bytes()
                ),
                (i, ai, f, af)
            );
            validate_decoded_stylesheet(&decision, bytes, &decoded).unwrap();
            rows += 1;
        }
    }
    eprintln!("AJ8 utf8 partition rows={rows}");
}
#[test]
fn aj8_a2_finalized_and_interrupted_accounting_are_distinct() {
    let body = b"\xff\xff";
    let d = selected(body, None);
    let outcome = decode_with_partitions(body, d, &[2], Some(3), false).unwrap();
    assert_eq!(outcome.text(), Err(CssDecodeFailure::OutputBound));
    let CssDecodeReport::Interrupted(r) = outcome.report() else {
        panic!("interrupted output")
    };
    // Both input bytes were decoded despite only the first callback being appended.
    assert_eq!(r.payload_bytes_processed(), 2);
    assert_eq!(r.emitted_utf8_bytes(), 3);
    assert_eq!(r.replacements().unwrap().invalid_events(), 2);
    assert!(!r.eof_processed());
    validate_decoded_stylesheet(&d, body, &outcome).unwrap();
    let eof_failure =
        decode_with_partitions(b"\xe2", selected(b"\xe2", None), &[1], Some(0), false).unwrap();
    let CssDecodeReport::Interrupted(r) = eof_failure.report() else {
        panic!("EOF output failure")
    };
    assert!(r.eof_processed());
    assert_eq!(r.pending_payload_bytes(), Some(0));
    let refused = decode_with_partitions(body, d, &[2], None, true).unwrap();
    assert_eq!(refused.report(), CssDecodeReport::NotStarted);
    assert_eq!(refused.text(), Err(CssDecodeFailure::OutputReservation));
}
#[test]
fn aj8_replacement_expansion_keeps_existing_parser_boundary() {
    let body = vec![0xff; MAX_STYLESHEET_BYTES];
    let d = selected(&body, None);
    let output = decode_stylesheet(&body, d).unwrap();
    assert_eq!(output.text().unwrap().len(), 6 * 1024 * 1024);
    validate_decoded_stylesheet(&d, &body, &output).unwrap();
}
