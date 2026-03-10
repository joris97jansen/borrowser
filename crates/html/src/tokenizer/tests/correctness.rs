use super::super::tokenize;
use super::helpers::text_eq;
use crate::types::{AttributeValue, Token};

#[test]
fn tokenize_preserves_utf8_text_nodes() {
    let stream = tokenize("<p>120×32</p>");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "120×32")),
        "expected UTF-8 text token, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_uppercase_doctype() {
    let stream = tokenize("<!DOCTYPE html>");
    assert!(
        stream.iter().any(|t| matches!(t, Token::Doctype(s)
                if stream.payload_text(s) == "DOCTYPE html")),
        "expected case-insensitive doctype, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_mixed_case_doctype() {
    let stream = tokenize("<!DoCtYpE html>");
    assert!(
        stream.iter().any(|t| matches!(t, Token::Doctype(s)
                if stream.payload_text(s) == "DoCtYpE html")),
        "expected mixed-case doctype to parse, got: {stream:?}"
    );
}

#[test]
fn tokenize_trims_doctype_whitespace_with_utf8() {
    let stream = tokenize("<!DOCTYPE  café  >");
    assert!(
        stream.iter().any(|t| matches!(t, Token::Doctype(s)
                if stream.payload_text(s) == "DOCTYPE  café")),
        "expected trimmed doctype payload, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_non_ascii_text_around_tags() {
    let stream = tokenize("¡Hola <b>café</b> 😊");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "¡Hola ")),
        "expected leading UTF-8 text token, got: {stream:?}"
    );
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "café")),
        "expected UTF-8 text inside tag, got: {stream:?}"
    );
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, " 😊")),
        "expected trailing UTF-8 text token, got: {stream:?}"
    );
}

#[test]
fn tokenize_accepts_end_tag_with_trailing_junk() {
    let stream = tokenize("</div foo>");
    let atoms = stream.atoms();
    assert!(
        matches!(stream.tokens(), [Token::EndTag(name)] if atoms.resolve(*name) == "div"),
        "expected end tag to ignore trailing junk, got: {stream:?}"
    );
}

#[test]
fn tokenize_accepts_attributes_after_invalid_name_char() {
    let stream = tokenize("<div @id=one></div>");
    let atoms = stream.atoms();
    assert!(
        stream.iter().any(|t| matches!(
            t,
            Token::StartTag { name, attributes, .. }
                if atoms.resolve(*name) == "div"
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "id"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("one")
                    })
        )),
        "expected permissive attribute parsing after invalid name char, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_non_ascii_attribute_values() {
    let stream = tokenize("<p data=naïve>ok</p>");
    let atoms = stream.atoms();
    assert!(
        stream.iter().any(|t| matches!(
            t,
            Token::StartTag { name, attributes, .. }
                if atoms.resolve(*name) == "p"
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "data"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("naïve")
                    })
        )),
        "expected UTF-8 attribute value, got: {stream:?}"
    );
}

#[test]
fn tokenize_decodes_entities_in_unquoted_attributes() {
    let stream = tokenize("<p data=Tom&amp;Jerry title=&#x3C;ok&#x3E;>ok</p>");
    let atoms = stream.atoms();
    assert!(
        stream.iter().any(|t| matches!(
            t,
            Token::StartTag { name, attributes, .. }
                if atoms.resolve(*name) == "p"
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "data"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("Tom&Jerry")
                    })
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "title"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("<ok>")
                    })
        )),
        "expected entity-decoded unquoted attributes, got: {stream:?}"
    );
}

#[test]
fn tokenize_attribute_values_use_span_when_unchanged() {
    let stream = tokenize("<p data=plain title=\"also-plain\" data-empty=>ok</p>");
    let atoms = stream.atoms();
    let mut spans = 0usize;
    for token in stream.iter() {
        if let Token::StartTag {
            name, attributes, ..
        } = token
            && atoms.resolve(*name) == "p"
        {
            for (key, value) in attributes {
                let key_name = atoms.resolve(*key);
                if (key_name.starts_with("data") || key_name == "title")
                    && matches!(value, Some(AttributeValue::Span { .. }))
                {
                    spans += 1;
                }
            }
        }
    }
    assert!(
        spans >= 2,
        "expected unchanged attribute values to use spans, got {spans}"
    );
}

#[test]
fn tokenize_attribute_values_allocate_when_decoded() {
    let stream = tokenize("<p data=Tom&amp;Jerry>ok</p>");
    let atoms = stream.atoms();
    let mut owned = 0usize;
    for token in stream.iter() {
        if let Token::StartTag {
            name, attributes, ..
        } = token
            && atoms.resolve(*name) == "p"
        {
            for (key, value) in attributes {
                if atoms.resolve(*key) == "data" && matches!(value, Some(AttributeValue::Owned(_)))
                {
                    owned += 1;
                }
            }
        }
    }
    assert!(
        owned >= 1,
        "expected decoded attribute value to allocate, got {owned}"
    );
}

#[test]
fn tokenize_text_preserves_literal_ampersand() {
    let stream = tokenize("<p>Tom&Jerry</p>");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "Tom&Jerry")),
        "expected literal '&' text to remain unchanged, got: {stream:?}"
    );
}

#[test]
fn tokenize_text_decodes_entities() {
    let stream = tokenize("<p>Tom&amp;Jerry</p>");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "Tom&Jerry")),
        "expected entity-decoded text, got: {stream:?}"
    );
    assert!(
        stream.iter().any(|t| matches!(t, Token::TextOwned { .. })),
        "expected decoded text to be owned, got: {stream:?}"
    );
}

#[test]
fn tokenize_text_preserves_malformed_entities() {
    let stream = tokenize("<p>Tom&amp</p><p>&#xZZ;</p><p>&unknown;</p>");
    let texts: Vec<&str> = stream.iter().filter_map(|t| stream.text(t)).collect();
    assert!(
        texts.contains(&"Tom&amp"),
        "expected incomplete entity to remain unchanged, got: {texts:?}"
    );
    assert!(
        texts.contains(&"&#xZZ;"),
        "expected malformed numeric entity to remain unchanged, got: {texts:?}"
    );
    assert!(
        texts.contains(&"&unknown;"),
        "expected unknown entity to remain unchanged, got: {texts:?}"
    );
}

#[test]
fn tokenize_handles_utf8_adjacent_to_angle_brackets() {
    let stream = tokenize("é<b>ï</b>ö");
    assert!(stream.iter().any(|t| text_eq(&stream, t, "é")));
    assert!(stream.iter().any(|t| text_eq(&stream, t, "ï")));
    assert!(stream.iter().any(|t| text_eq(&stream, t, "ö")));
}

#[test]
fn tokenize_interns_case_insensitive_tag_and_attr_names() {
    let stream = tokenize("<DiV id=one></div><div ID=two></DIV>");
    let atoms = stream.atoms();
    let mut div_ids = Vec::new();
    let mut id_ids = Vec::new();

    for token in stream.iter() {
        match token {
            Token::StartTag {
                name, attributes, ..
            } => {
                div_ids.push(*name);
                for (attr_name, _) in attributes {
                    id_ids.push(*attr_name);
                }
            }
            Token::EndTag(name) => div_ids.push(*name),
            _ => {}
        }
    }

    assert!(
        div_ids.windows(2).all(|w| w[0] == w[1]),
        "expected all div atoms to match, got: {div_ids:?}"
    );
    assert!(
        id_ids.windows(2).all(|w| w[0] == w[1]),
        "expected all id atoms to match, got: {id_ids:?}"
    );
    assert_eq!(atoms.resolve(div_ids[0]), "div");
    assert_eq!(atoms.resolve(id_ids[0]), "id");
    assert_eq!(atoms.len(), 2, "expected only two interned names");
}

#[test]
fn tokenize_allows_custom_element_and_namespaced_tags() {
    let stream = tokenize("<my-component></my-component><svg:rect></svg:rect>");
    let atoms = stream.atoms();
    let mut names = Vec::new();

    for token in stream.iter() {
        match token {
            Token::StartTag { name, .. } | Token::EndTag(name) => names.push(*name),
            _ => {}
        }
    }

    assert_eq!(atoms.resolve(names[0]), "my-component");
    assert_eq!(atoms.resolve(names[1]), "my-component");
    assert_eq!(atoms.resolve(names[2]), "svg:rect");
    assert_eq!(atoms.resolve(names[3]), "svg:rect");
}

#[test]
fn tokenize_handles_many_simple_tags_linearly() {
    let mut input = String::new();
    for _ in 0..20_000 {
        input.push_str("<a></a>");
    }
    let stream = tokenize(&input);
    assert_eq!(stream.tokens().len(), 40_000);
}

#[test]
fn tokenize_handles_many_comments_and_doctypes() {
    let mut input = String::new();
    for _ in 0..5_000 {
        input.push_str("<!--x-->");
    }
    for _ in 0..5_000 {
        input.push_str("<!DOCTYPE html>");
    }

    let stream = tokenize(&input);
    let mut comment_count = 0;
    let mut doctype_count = 0;
    for token in stream.iter() {
        match token {
            Token::Comment(_) => comment_count += 1,
            Token::Doctype(_) => doctype_count += 1,
            _ => {}
        }
    }

    assert_eq!(comment_count, 5_000);
    assert_eq!(doctype_count, 5_000);
}

#[test]
fn tokenize_does_not_emit_empty_text_tokens() {
    let stream = tokenize("<p></p>");
    assert!(
        !stream
            .tokens()
            .iter()
            .any(|t| matches!(t, Token::TextSpan { .. } | Token::TextOwned { .. })),
        "expected no text tokens for empty element, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_tons_of_angle_brackets() {
    let input = "<".repeat(200_000);
    let stream = tokenize(&input);
    assert!(stream.tokens().len() <= input.len());
}
