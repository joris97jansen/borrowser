use std::borrow::Cow;

use super::decode_entities;
use super::numeric::scan_numeric_entity;

#[cfg(feature = "html5-entities")]
use super::html5::HTML5_ENTITIES;
#[cfg(feature = "html5-entities")]
use super::policy::decode_entities_html5_in_attribute as decode_entities_html5_attr;
#[cfg(feature = "html5-entities")]
use super::policy::decode_entities_html5_in_text as decode_entities_html5_text;

#[test]
fn scan_numeric_entity_enforces_digit_boundaries() {
    let bytes = b"12;";
    assert_eq!(scan_numeric_entity(bytes, 0, 2, false), Some(2));

    let bytes = b"x12;";
    assert_eq!(scan_numeric_entity(bytes, 1, 2, false), Some(3));

    let bytes = b"123;";
    assert_eq!(scan_numeric_entity(bytes, 0, 2, false), None);

    let input = "&#x1234567;";
    assert_eq!(decode_entities(input).as_ref(), input);

    let input = "&#x1234567;&amp;";
    assert_eq!(decode_entities(input).as_ref(), "&#x1234567;&");

    let input = "&#12345678;&amp;";
    assert_eq!(decode_entities(input).as_ref(), "&#12345678;&");
}

#[test]
fn decode_entities_preserves_utf8() {
    assert_eq!(decode_entities("120×32").as_ref(), "120×32");
}

#[test]
fn decode_entities_decodes_common_entities() {
    assert_eq!(decode_entities("a &amp; b").as_ref(), "a & b");
    assert_eq!(decode_entities("&lt;tag&gt;").as_ref(), "<tag>");
    assert_eq!(decode_entities("&quot;hi&quot;").as_ref(), "\"hi\"");
    assert_eq!(decode_entities("&apos;x&apos;").as_ref(), "'x'");
    assert_eq!(decode_entities("a&nbsp;b").as_ref(), "a\u{00A0}b");
}

#[cfg(feature = "html5-entities")]
#[test]
fn decode_entities_html5_decodes_named_entities() {
    let samples = [
        ("&AElig;", "Æ"),
        ("&NotEqualTilde;", "\u{2242}\u{0338}"),
        ("&NotSubset;", "\u{2282}\u{20D2}"),
    ];
    for (input, expected) in samples {
        assert_eq!(decode_entities_html5_text(input).as_ref(), expected);
    }
}

#[cfg(feature = "html5-entities")]
#[test]
fn decode_entities_html5_longest_match_semicolon() {
    let samples = [("&notin;", "∉"), ("&not;", "¬")];
    for (input, expected) in samples {
        assert_eq!(decode_entities_html5_text(input).as_ref(), expected);
    }
}

#[cfg(feature = "html5-entities")]
#[test]
fn decode_entities_html5_legacy_no_semicolon() {
    let samples = [("&AElig", "Æ"), ("&copy", "©"), ("x&copyy", "x©y")];
    for (input, expected) in samples {
        assert_eq!(decode_entities_html5_text(input).as_ref(), expected);
    }
}

#[cfg(feature = "html5-entities")]
#[test]
fn decode_entities_html5_attribute_value_restrictions() {
    assert_eq!(decode_entities_html5_attr("x&AEligy").as_ref(), "x&AEligy");
    assert_eq!(decode_entities_html5_attr("x&copy=1").as_ref(), "x&copy=1");
}

#[test]
fn decode_entities_decodes_numeric_entities() {
    assert_eq!(decode_entities("&#215;").as_ref(), "×");
    assert_eq!(decode_entities("&#xD7;").as_ref(), "×");
}

#[test]
fn decode_entities_utf8_then_entity() {
    assert_eq!(decode_entities("π &amp; σ").as_ref(), "π & σ");
}

#[test]
fn decode_entities_passes_through_unknown_and_missing_semicolon() {
    assert_eq!(
        decode_entities("before &notanentity; after").as_ref(),
        "before &notanentity; after"
    );
    assert_eq!(decode_entities("&amp").as_ref(), "&amp");
    assert_eq!(
        decode_entities("loose &amp space").as_ref(),
        "loose &amp space"
    );
    assert_eq!(decode_entities("&#xD7 ").as_ref(), "&#xD7 ");
    assert_eq!(decode_entities("&#215 ").as_ref(), "&#215 ");
}

#[test]
fn decode_entities_passes_through_malformed_numeric() {
    assert_eq!(decode_entities("&#xZZ;").as_ref(), "&#xZZ;");
    assert_eq!(decode_entities("&#99999999;").as_ref(), "&#99999999;");
    assert_eq!(decode_entities("&#xD800;").as_ref(), "&#xD800;");
    assert_eq!(decode_entities("&#x110000;").as_ref(), "&#x110000;");
    assert_eq!(decode_entities("&#-1;").as_ref(), "&#-1;");
    assert_eq!(decode_entities("&#x-1;").as_ref(), "&#x-1;");
    assert_eq!(decode_entities("&#12345678").as_ref(), "&#12345678");
    assert_eq!(decode_entities("&#123").as_ref(), "&#123");
    assert_eq!(decode_entities("&#;").as_ref(), "&#;");
    assert_eq!(decode_entities("&#x;").as_ref(), "&#x;");
}

#[test]
fn decode_entities_handles_long_numeric_like_patterns() {
    let noisy = "&#123456789;".repeat(100);
    assert_eq!(decode_entities(&noisy).as_ref(), noisy);
}

#[test]
fn decode_entities_respects_numeric_digit_limits() {
    assert_eq!(decode_entities("&#1114111;").as_ref(), "\u{10FFFF}");
    assert_eq!(decode_entities("&#11141111;").as_ref(), "&#11141111;");
    assert_eq!(decode_entities("&#x10FFFF;").as_ref(), "\u{10FFFF}");
    assert_eq!(decode_entities("&#x110000;").as_ref(), "&#x110000;");
}

#[test]
fn decode_entities_rejects_invalid_scalars() {
    assert_eq!(decode_entities("&#xD800;").as_ref(), "&#xD800;");
    assert_eq!(decode_entities("&#xDFFF;").as_ref(), "&#xDFFF;");
    assert_eq!(decode_entities("&#55296;").as_ref(), "&#55296;");
}

#[test]
fn decode_entities_property_like_adversarial_inputs() {
    let samples = [
        "&",
        "&&",
        "&;",
        "&#;",
        "&#x;",
        "&#xFFFFFFFF;",
        "&unknown;",
        "&#9999999;",
        "&amp;&lt;&gt;&quot;&apos;&nbsp;",
    ];

    for s in samples {
        let out = decode_entities(s).into_owned();
        assert_eq!(decode_entities(&out).as_ref(), out);
    }

    let unchanged = [
        "",
        "plain text",
        "πσ",
        "&",
        "&&",
        "&;",
        "&#;",
        "&#x;",
        "&unknown;",
        "&#xZZ;",
        "&#9999999;",
    ];

    for s in unchanged {
        assert_eq!(decode_entities(s).as_ref(), s);
    }
}

#[test]
fn decode_entities_regression_corpus_no_panic_and_idempotent() {
    let samples = [
        "&&&&&&&",
        "&&&&&&&&&&&&&&&&&amp;",
        "& &&& &&",
        "&#&#&#&#",
        "&#x&#x&#x",
        "a&b&c&d&e",
        "&#123456789012345678901234567890;",
        "&#xFFFFFFFFFFFFFFFFFFFFFFFF;",
        "end&#1234567;tail",
        "lead&#x10FFFF;trail",
        "mix&;ed&unknown;stuff",
        "a&amp;b&c&amp;d",
        "text &#xD7; more",
        "&#x10FFFF;&amp;&#1114111;",
        "&#11141111;&amp;&&",
        "&#1234567x;",
        "&#x10FFFFG;",
    ];

    for s in samples {
        let out = decode_entities(s);
        assert_eq!(decode_entities(out.as_ref()).as_ref(), out.as_ref());
    }
}

#[test]
fn decode_entities_regression_corpus_utf8_boundaries() {
    let samples = [
        "&\u{00A0}&\u{00A0}&",
        "π&σ&&amp;&",
        "utf8×&amp;σ",
        "π&\u{00A0}σ",
    ];

    for s in samples {
        let out = decode_entities(s);
        assert_eq!(decode_entities(out.as_ref()).as_ref(), out.as_ref());
    }
}

#[cfg(feature = "html5-entities")]
#[test]
fn decode_entities_html5_semicolon_terminated_samples() {
    let stride = (HTML5_ENTITIES.len() / 64).max(1);
    for i in (0..HTML5_ENTITIES.len()).step_by(stride) {
        let entity = &HTML5_ENTITIES[i];
        let name = std::str::from_utf8(entity.name).expect("ascii entity name");
        let input = format!("x{name}y");
        let expected = format!("x{}y", entity.value);
        assert_eq!(decode_entities_html5_text(&input).as_ref(), expected);
    }
}

#[test]
fn malformed_entity_allows_following_entity() {
    assert_eq!(decode_entities("&#xZZ;&amp;").as_ref(), "&#xZZ;&");
}

#[test]
fn decode_entities_returns_borrowed_when_no_entities() {
    let out = decode_entities("plain text");
    assert!(matches!(out, Cow::Borrowed(_)));
    assert_eq!(out.as_ref(), "plain text");
}

#[test]
fn decode_entities_borrows_when_ampersand_has_no_decodable_entity() {
    let samples = ["hello & world", "a &amp b", "&#xZZ;", "&unknown;"];
    for s in samples {
        let out = decode_entities(s);
        assert!(matches!(out, Cow::Borrowed(_)), "expected borrowed for {s}");
        assert_eq!(out.as_ref(), s);
    }
}

#[test]
fn decode_entities_owns_when_decoding_occurs() {
    let samples = [
        ("Tom&amp;Jerry", "Tom&Jerry"),
        ("&#215;", "×"),
        ("&#xD7;", "×"),
    ];
    for (input, expected) in samples {
        let out = decode_entities(input);
        assert!(matches!(out, Cow::Owned(_)), "expected owned for {input}");
        assert_eq!(out.as_ref(), expected);
    }
}
