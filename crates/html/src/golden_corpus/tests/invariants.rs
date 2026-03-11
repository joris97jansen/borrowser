use super::super::{Expectation, GoldenFixture, Invariant};
use crate::dom_snapshot::{DomSnapshotOptions, compare_dom};
use crate::{Node, Token, TokenStream};
use std::collections::BTreeMap;

pub(super) struct InvariantCtx<'a> {
    fixture: &'a GoldenFixture,
    full_dom: &'a Node,
    chunked_dom: &'a Node,
    chunked_tokens: &'a TokenStream,
}

impl<'a> InvariantCtx<'a> {
    pub(super) fn new(
        fixture: &'a GoldenFixture,
        full_dom: &'a Node,
        chunked_dom: &'a Node,
        chunked_tokens: &'a TokenStream,
    ) -> Self {
        Self {
            fixture,
            full_dom,
            chunked_dom,
            chunked_tokens,
        }
    }
}

pub(super) fn check_invariant(ctx: &InvariantCtx<'_>, invariant: Invariant) -> Result<(), String> {
    match invariant {
        Invariant::FullEqualsChunkedDom => {
            compare_dom(ctx.full_dom, ctx.chunked_dom, DomSnapshotOptions::default())
                .map_err(|err| err.to_string())
        }
        Invariant::HasDoctypeToken => check_doctype_token(ctx),
        Invariant::HasCommentToken => check_comment_token(ctx),
        Invariant::PreservesUtf8Text => check_utf8_text(ctx),
        Invariant::DecodesNamedEntities => check_named_entities(ctx),
        Invariant::DecodesNumericEntities => check_numeric_entities(ctx),
        Invariant::PartialEntityRemainsLiteral => check_partial_entity_literal(ctx),
        Invariant::AcceptsMixedAttributeSyntax
        | Invariant::AttributesParsedWithSpacing
        | Invariant::BooleanAttributePresent
        | Invariant::EmptyAttributeValuePreserved => {
            check_token_attributes(ctx.fixture.name, ctx.chunked_tokens)
        }
        Invariant::TagBoundariesStable => check_tag_boundaries(ctx),
        Invariant::CustomTagRecognized => check_tag_presence(ctx, "my-component", "custom tag"),
        Invariant::NamespacedTagRecognized => check_tag_presence(ctx, "svg:rect", "namespaced tag"),
        Invariant::ScriptRawtextVerbatim => check_script_rawtext_verbatim(ctx),
        Invariant::RawtextCloseTagRecognized => check_rawtext_close_tag(ctx),
        Invariant::RawtextNearMatchStaysText => check_rawtext_near_match(ctx),
    }
}

pub(super) fn allowed_failure_reason(
    fixture: &GoldenFixture,
    invariant: Invariant,
) -> Option<&'static str> {
    match fixture.expectation {
        Expectation::MustPass => None,
        Expectation::AllowedToFail { allowed } => allowed
            .iter()
            .find(|entry| entry.invariant == invariant)
            .map(|entry| entry.reason),
    }
}

fn check_doctype_token(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let full_has = matches!(ctx.full_dom, Node::Document { doctype, .. } if doctype.is_some());
    let chunked_has =
        matches!(ctx.chunked_dom, Node::Document { doctype, .. } if doctype.is_some());
    if full_has == chunked_has {
        Ok(())
    } else {
        Err(format!(
            "doctype parity mismatch: full={full_has} chunked={chunked_has}"
        ))
    }
}

fn check_comment_token(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let full_has = has_comment(ctx.full_dom);
    let chunked_has = has_comment(ctx.chunked_dom);
    if full_has == chunked_has {
        Ok(())
    } else {
        Err(format!(
            "comment parity mismatch: full={full_has} chunked={chunked_has}"
        ))
    }
}

fn check_utf8_text(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let expected_chars: Vec<char> = ctx
        .fixture
        .input
        .chars()
        .filter(|ch| !ch.is_ascii())
        .collect();
    if expected_chars.is_empty() {
        return Ok(());
    }
    let full_text = collect_text(ctx.full_dom);
    let chunked_text = collect_text(ctx.chunked_dom);
    for ch in expected_chars {
        let full_has = full_text.contains(ch);
        let chunked_has = chunked_text.contains(ch);
        if full_has != chunked_has {
            return Err(format!(
                "UTF-8 text parity mismatch for {ch}: full={full_has} chunked={chunked_has}"
            ));
        }
    }
    Ok(())
}

fn check_named_entities(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    if !ctx.fixture.input.contains("&amp;") {
        return Ok(());
    }
    let full_text = collect_text(ctx.full_dom);
    let chunked_text = collect_text(ctx.chunked_dom);
    let full_ok = full_text.contains('&') && !full_text.contains("&amp;");
    let chunked_ok = chunked_text.contains('&') && !chunked_text.contains("&amp;");
    if full_ok == chunked_ok {
        Ok(())
    } else {
        Err(format!(
            "named entity parity mismatch: full={full_ok} chunked={chunked_ok}"
        ))
    }
}

fn check_numeric_entities(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let full_text = collect_text(ctx.full_dom);
    let chunked_text = collect_text(ctx.chunked_dom);
    for (entity, expected_char, label) in [
        ("&#123;", '{', "&#123;"),
        ("&#169;", '©', "&#169;"),
        ("&#x1F600;", '😀', "&#x1F600;"),
    ] {
        if !ctx.fixture.input.contains(entity) {
            continue;
        }
        let full_ok = full_text.contains(expected_char);
        let chunked_ok = chunked_text.contains(expected_char);
        if full_ok != chunked_ok {
            return Err(format!(
                "numeric entity parity mismatch for {label}: full={full_ok} chunked={chunked_ok}"
            ));
        }
    }
    Ok(())
}

fn check_partial_entity_literal(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let full_ok = collect_text(ctx.full_dom).contains("&am");
    let chunked_ok = collect_text(ctx.chunked_dom).contains("&am");
    if full_ok == chunked_ok {
        Ok(())
    } else {
        Err(format!(
            "partial entity parity mismatch: full={full_ok} chunked={chunked_ok}"
        ))
    }
}

fn check_tag_boundaries(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let expected = element_name_counts(ctx.full_dom);
    let actual = element_name_counts(ctx.chunked_dom);
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "tag name parity mismatch: full={expected:?} chunked={actual:?}"
        ))
    }
}

fn check_tag_presence(ctx: &InvariantCtx<'_>, tag_name: &str, label: &str) -> Result<(), String> {
    let full_has = find_element(ctx.full_dom, tag_name).is_some();
    let chunked_has = find_element(ctx.chunked_dom, tag_name).is_some();
    if full_has == chunked_has {
        Ok(())
    } else {
        Err(format!(
            "{label} parity mismatch: full={full_has} chunked={chunked_has}"
        ))
    }
}

fn check_script_rawtext_verbatim(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let expected = script_body_from_input(ctx.fixture.input)
        .ok_or_else(|| "expected <script> body in fixture input".to_string())?;
    let full_actual = script_text(ctx.full_dom)
        .ok_or_else(|| "expected <script> element in full DOM".to_string())?;
    let chunked_actual = script_text(ctx.chunked_dom)
        .ok_or_else(|| "expected <script> element in chunked DOM".to_string())?;
    let full_ok = full_actual == expected;
    let chunked_ok = chunked_actual == expected;
    if full_ok == chunked_ok {
        Ok(())
    } else {
        Err(format!(
            "rawtext parity mismatch: full={full_ok} chunked={chunked_ok}"
        ))
    }
}

fn check_rawtext_close_tag(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let full_text = script_text(ctx.full_dom)
        .ok_or_else(|| "expected <script> element in full DOM".to_string())?;
    let chunked_text = script_text(ctx.chunked_dom)
        .ok_or_else(|| "expected <script> element in chunked DOM".to_string())?;
    let full_ok = full_text == "hi";
    let chunked_ok = chunked_text == "hi";
    if full_ok == chunked_ok {
        Ok(())
    } else {
        Err(format!(
            "rawtext close-tag parity mismatch: full={full_ok} chunked={chunked_ok}"
        ))
    }
}

fn check_rawtext_near_match(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    let full_text = script_text(ctx.full_dom)
        .ok_or_else(|| "expected <script> element in full DOM".to_string())?;
    let chunked_text = script_text(ctx.chunked_dom)
        .ok_or_else(|| "expected <script> element in chunked DOM".to_string())?;
    let full_ok = full_text.contains("</scriptx>");
    let chunked_ok = chunked_text.contains("</scriptx>");
    if full_ok == chunked_ok {
        Ok(())
    } else {
        Err(format!(
            "rawtext near-match parity mismatch: full={full_ok} chunked={chunked_ok}"
        ))
    }
}

fn collect_text(node: &Node) -> String {
    let mut out = String::new();
    collect_text_into(node, &mut out);
    out
}

fn collect_text_into(node: &Node, out: &mut String) {
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                collect_text_into(child, out);
            }
        }
        Node::Text { text, .. } => out.push_str(text),
        Node::Comment { .. } => {}
    }
}

fn has_comment(node: &Node) -> bool {
    match node {
        Node::Comment { .. } => true,
        Node::Document { children, .. } | Node::Element { children, .. } => {
            children.iter().any(has_comment)
        }
        Node::Text { .. } => false,
    }
}

fn find_element<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    match node {
        Node::Element { name: tag, .. } => {
            crate::types::debug_assert_lowercase_atom(tag, "golden find_element tag");
            if tag.as_ref() == name {
                Some(node)
            } else {
                None
            }
        }
        Node::Document { children, .. } => {
            children.iter().find_map(|child| find_element(child, name))
        }
        Node::Text { .. } | Node::Comment { .. } => None,
    }
}

fn script_text(node: &Node) -> Option<String> {
    let script = find_element(node, "script")?;
    let Node::Element { children, .. } = script else {
        return None;
    };
    let mut out = String::new();
    for child in children {
        collect_text_into(child, &mut out);
    }
    Some(out)
}

fn script_body_from_input(input: &str) -> Option<&str> {
    let start = input.find("<script>")?;
    let end = input.rfind("</script>")?;
    let body_start = start + "<script>".len();
    if body_start > end {
        return None;
    }
    Some(&input[body_start..end])
}

fn element_name_counts(node: &Node) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    collect_element_names(node, &mut out);
    out
}

fn collect_element_names(node: &Node, out: &mut BTreeMap<String, usize>) {
    match node {
        Node::Element { name, children, .. } => {
            crate::types::debug_assert_lowercase_atom(name, "golden element name");
            *out.entry(name.to_string()).or_insert(0) += 1;
            for child in children {
                collect_element_names(child, out);
            }
        }
        Node::Document { children, .. } => {
            for child in children {
                collect_element_names(child, out);
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn check_token_attributes(fixture_name: &str, stream: &TokenStream) -> Result<(), String> {
    type StartTagAttrs<'a> = (
        &'a str,
        &'a [(crate::AtomId, Option<crate::AttributeValue>)],
    );

    let atoms = stream.atoms();
    let start_tags: Vec<StartTagAttrs<'_>> = stream
        .tokens()
        .iter()
        .filter_map(|token| match token {
            Token::StartTag {
                name, attributes, ..
            } => Some((atoms.resolve(*name), attributes.as_slice())),
            _ => None,
        })
        .collect();

    match fixture_name {
        "attr_quoted_unquoted" => {
            let (_, attrs) = start_tags
                .iter()
                .find(|(tag, _)| *tag == "div")
                .ok_or_else(|| "expected <div> start tag".to_string())?;
            let class = find_attr(stream, atoms, attrs, "class");
            let data_x = find_attr(stream, atoms, attrs, "data-x");
            if class == Some("a b") && data_x == Some("1") {
                Ok(())
            } else {
                Err(format!(
                    "expected class=\"a b\" and data-x=\"1\", got class={class:?} data-x={data_x:?}"
                ))
            }
        }
        "attr_quote_variants" => {
            let (_, attrs) = start_tags
                .iter()
                .find(|(tag, _)| *tag == "input")
                .ok_or_else(|| "expected <input> start tag".to_string())?;
            let value = find_attr(stream, atoms, attrs, "value");
            let title = find_attr(stream, atoms, attrs, "title");
            if value == Some("a b") && title == Some("c d") {
                Ok(())
            } else {
                Err("expected value=\"a b\" and title=\"c d\"".to_string())
            }
        }
        "attr_whitespace_variations" => {
            let (_, attrs) = start_tags
                .iter()
                .find(|(tag, _)| *tag == "div")
                .ok_or_else(|| "expected <div> start tag".to_string())?;
            let id = find_attr(stream, atoms, attrs, "id");
            let class = find_attr(stream, atoms, attrs, "class");
            if id == Some("a") && class == Some("foo") {
                Ok(())
            } else {
                Err("expected id=\"a\" and class=\"foo\"".to_string())
            }
        }
        "attr_boolean_empty" => {
            let (_, attrs) = start_tags
                .iter()
                .find(|(tag, _)| *tag == "input")
                .ok_or_else(|| "expected <input> start tag".to_string())?;
            let disabled_present = has_attr(atoms, attrs, "disabled");
            let required_present = has_attr(atoms, attrs, "required");
            let data_empty = find_attr(stream, atoms, attrs, "data-empty");
            if disabled_present && required_present && data_empty == Some("") {
                Ok(())
            } else {
                Err("expected disabled+required boolean attrs and data-empty=\"\"".to_string())
            }
        }
        _ => Err(format!(
            "attribute expectations not defined for fixture: {fixture_name}"
        )),
    }
}

fn find_attr<'a>(
    stream: &'a TokenStream,
    atoms: &'a crate::AtomTable,
    attrs: &'a [(crate::AtomId, Option<crate::AttributeValue>)],
    name: &str,
) -> Option<&'a str> {
    attrs.iter().find_map(|(key, value)| {
        let key_name = atoms.resolve(*key);
        crate::types::debug_assert_lowercase_atom(key_name, "golden attribute name");
        if key_name == name {
            Some(
                value
                    .as_ref()
                    .map(|value| stream.attr_value(value))
                    .unwrap_or(""),
            )
        } else {
            None
        }
    })
}

fn has_attr(
    atoms: &crate::AtomTable,
    attrs: &[(crate::AtomId, Option<crate::AttributeValue>)],
    name: &str,
) -> bool {
    attrs.iter().any(|(key, _)| {
        let key_name = atoms.resolve(*key);
        crate::types::debug_assert_lowercase_atom(key_name, "golden attribute name");
        key_name == name
    })
}
