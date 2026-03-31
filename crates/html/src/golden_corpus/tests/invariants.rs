use super::super::{Expectation, GoldenFixture, Invariant};
use crate::Node;
use crate::dom_snapshot::{DomSnapshotOptions, compare_dom};
use std::collections::BTreeMap;

pub(super) struct InvariantCtx<'a> {
    fixture: &'a GoldenFixture,
    full_dom: &'a Node,
    chunked_dom: &'a Node,
}

impl<'a> InvariantCtx<'a> {
    pub(super) fn new(
        fixture: &'a GoldenFixture,
        full_dom: &'a Node,
        chunked_dom: &'a Node,
    ) -> Self {
        Self {
            fixture,
            full_dom,
            chunked_dom,
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
        | Invariant::EmptyAttributeValuePreserved => check_attribute_parsing(ctx),
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
        Node::Element {
            name: tag,
            children,
            ..
        } => {
            crate::types::debug_assert_lowercase_atom(tag, "golden find_element tag");
            if tag.as_ref() == name {
                Some(node)
            } else {
                children.iter().find_map(|child| find_element(child, name))
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

fn check_attribute_parsing(ctx: &InvariantCtx<'_>) -> Result<(), String> {
    match ctx.fixture.name {
        "attr_quoted_unquoted" => {
            let full_attrs = element_attributes(ctx.full_dom, "div")
                .ok_or_else(|| "expected <div> element in full DOM".to_string())?;
            let chunked_attrs = element_attributes(ctx.chunked_dom, "div")
                .ok_or_else(|| "expected <div> element in chunked DOM".to_string())?;
            let full_class = find_attr(full_attrs, "class");
            let full_data_x = find_attr(full_attrs, "data-x");
            let chunked_class = find_attr(chunked_attrs, "class");
            let chunked_data_x = find_attr(chunked_attrs, "data-x");
            if full_class == Some("a b")
                && full_data_x == Some("1")
                && chunked_class == Some("a b")
                && chunked_data_x == Some("1")
            {
                Ok(())
            } else {
                Err(format!(
                    "expected class=\"a b\" and data-x=\"1\", got full_class={full_class:?} full_data_x={full_data_x:?} chunked_class={chunked_class:?} chunked_data_x={chunked_data_x:?}"
                ))
            }
        }
        "attr_quote_variants" => {
            let full_attrs = element_attributes(ctx.full_dom, "input")
                .ok_or_else(|| "expected <input> element in full DOM".to_string())?;
            let chunked_attrs = element_attributes(ctx.chunked_dom, "input")
                .ok_or_else(|| "expected <input> element in chunked DOM".to_string())?;
            let full_value = find_attr(full_attrs, "value");
            let full_title = find_attr(full_attrs, "title");
            let chunked_value = find_attr(chunked_attrs, "value");
            let chunked_title = find_attr(chunked_attrs, "title");
            if full_value == Some("a b")
                && full_title == Some("c d")
                && chunked_value == Some("a b")
                && chunked_title == Some("c d")
            {
                Ok(())
            } else {
                Err(format!(
                    "expected value=\"a b\" and title=\"c d\", got full_value={full_value:?} full_title={full_title:?} chunked_value={chunked_value:?} chunked_title={chunked_title:?}"
                ))
            }
        }
        "attr_whitespace_variations" => {
            let full_attrs = element_attributes(ctx.full_dom, "div")
                .ok_or_else(|| "expected <div> element in full DOM".to_string())?;
            let chunked_attrs = element_attributes(ctx.chunked_dom, "div")
                .ok_or_else(|| "expected <div> element in chunked DOM".to_string())?;
            let full_id = find_attr(full_attrs, "id");
            let full_class = find_attr(full_attrs, "class");
            let chunked_id = find_attr(chunked_attrs, "id");
            let chunked_class = find_attr(chunked_attrs, "class");
            if full_id == Some("a")
                && full_class == Some("foo")
                && chunked_id == Some("a")
                && chunked_class == Some("foo")
            {
                Ok(())
            } else {
                Err(format!(
                    "expected id=\"a\" and class=\"foo\", got full_id={full_id:?} full_class={full_class:?} chunked_id={chunked_id:?} chunked_class={chunked_class:?}"
                ))
            }
        }
        "attr_boolean_empty" => {
            let full_attrs = element_attributes(ctx.full_dom, "input")
                .ok_or_else(|| "expected <input> element in full DOM".to_string())?;
            let chunked_attrs = element_attributes(ctx.chunked_dom, "input")
                .ok_or_else(|| "expected <input> element in chunked DOM".to_string())?;
            let full_disabled = has_attr(full_attrs, "disabled");
            let full_required = has_attr(full_attrs, "required");
            let full_data_empty = find_attr(full_attrs, "data-empty");
            let chunked_disabled = has_attr(chunked_attrs, "disabled");
            let chunked_required = has_attr(chunked_attrs, "required");
            let chunked_data_empty = find_attr(chunked_attrs, "data-empty");
            if full_disabled
                && full_required
                && full_data_empty == Some("")
                && chunked_disabled
                && chunked_required
                && chunked_data_empty == Some("")
            {
                Ok(())
            } else {
                Err(format!(
                    "expected disabled+required boolean attrs and data-empty=\"\", got full_disabled={full_disabled} full_required={full_required} full_data_empty={full_data_empty:?} chunked_disabled={chunked_disabled} chunked_required={chunked_required} chunked_data_empty={chunked_data_empty:?}"
                ))
            }
        }
        _ => Err(format!(
            "attribute expectations not defined for fixture: {}",
            ctx.fixture.name
        )),
    }
}

fn element_attributes<'a>(
    node: &'a Node,
    tag_name: &str,
) -> Option<&'a [(std::sync::Arc<str>, Option<String>)]> {
    match find_element(node, tag_name)? {
        Node::Element { attributes, .. } => Some(attributes.as_slice()),
        Node::Document { .. } | Node::Text { .. } | Node::Comment { .. } => None,
    }
}

fn find_attr<'a>(
    attrs: &'a [(std::sync::Arc<str>, Option<String>)],
    name: &str,
) -> Option<&'a str> {
    attrs.iter().find_map(|(key, value)| {
        crate::types::debug_assert_lowercase_atom(key, "golden attribute name");
        if key.as_ref() == name {
            Some(value.as_deref().unwrap_or(""))
        } else {
            None
        }
    })
}

fn has_attr(attrs: &[(std::sync::Arc<str>, Option<String>)], name: &str) -> bool {
    attrs.iter().any(|(key, _)| {
        crate::types::debug_assert_lowercase_atom(key, "golden attribute name");
        key.as_ref() == name
    })
}
