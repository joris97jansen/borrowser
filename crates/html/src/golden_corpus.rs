#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Invariant {
    FullEqualsChunkedDom,
    DecodesNamedEntities,
    DecodesNumericEntities,
    PreservesUtf8Text,
    ScriptRawtextVerbatim,
    AcceptsMixedAttributeSyntax,
    HasDoctypeToken,
    HasCommentToken,
    TagBoundariesStable,
    PartialEntityRemainsLiteral,
    RawtextCloseTagRecognized,
    RawtextNearMatchStaysText,
    CustomTagRecognized,
    NamespacedTagRecognized,
    AttributesParsedWithSpacing,
    EmptyAttributeValuePreserved,
    BooleanAttributePresent,
}

impl Invariant {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FullEqualsChunkedDom => "full equals chunked dom",
            Self::DecodesNamedEntities => "decodes named entities",
            Self::DecodesNumericEntities => "decodes numeric entities",
            Self::PreservesUtf8Text => "preserves utf-8 text",
            Self::ScriptRawtextVerbatim => "script rawtext verbatim",
            Self::AcceptsMixedAttributeSyntax => "accepts mixed attribute syntax",
            Self::HasDoctypeToken => "has doctype token",
            Self::HasCommentToken => "has comment token",
            Self::TagBoundariesStable => "tag boundaries stable",
            Self::PartialEntityRemainsLiteral => "partial entity remains literal",
            Self::RawtextCloseTagRecognized => "rawtext close tag recognized",
            Self::RawtextNearMatchStaysText => "rawtext near match stays text",
            Self::CustomTagRecognized => "custom tag recognized",
            Self::NamespacedTagRecognized => "namespaced tag recognized",
            Self::AttributesParsedWithSpacing => "attributes parsed with spacing",
            Self::EmptyAttributeValuePreserved => "empty attribute value preserved",
            Self::BooleanAttributePresent => "boolean attribute present",
        }
    }
}

impl std::fmt::Display for Invariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expectation {
    MustPass,
    AllowedToFail { allowed: &'static [AllowedFailure] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllowedFailure {
    pub invariant: Invariant,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FixtureKind {
    Utf8,
    Entity,
    Attribute,
    Comment,
    Doctype,
    Rawtext,
    TagName,
}

#[derive(Clone, Copy, Debug)]
pub struct GoldenFixture {
    pub name: &'static str,
    pub input: &'static str,
    pub covers: &'static str,
    pub tags: &'static [&'static str],
    pub invariants: &'static [Invariant],
    pub expectation: Expectation,
    pub kind: FixtureKind,
}

const GOLDEN_CORPUS_V1: &[GoldenFixture] = &[
    GoldenFixture {
        name: "utf8_non_ascii_tags",
        input: "é<b>ï</b>ö",
        covers: "Non-ASCII text around tags.",
        tags: &["utf8", "text", "tags"],
        invariants: &[Invariant::PreservesUtf8Text, Invariant::TagBoundariesStable],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Utf8,
    },
    GoldenFixture {
        name: "utf8_literal_gt_after_element",
        input: "é<em>ï</em>ö>",
        covers: "Trailing literal `>` after element close tag; must remain text.",
        tags: &["utf8", "text", "literal-gt"],
        invariants: &[Invariant::PreservesUtf8Text, Invariant::TagBoundariesStable],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Utf8,
    },
    GoldenFixture {
        name: "entity_named_amp",
        input: "<p>Tom &amp; Jerry</p>",
        covers: "Named entity decoding in text.",
        tags: &["entity", "named", "text"],
        invariants: &[
            Invariant::DecodesNamedEntities,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "entity_numeric",
        input: "<p>&#123;&#x1F600;</p>",
        covers: "Numeric and hex entities.",
        tags: &["entity", "numeric", "text"],
        invariants: &[
            Invariant::DecodesNumericEntities,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "entity_partial",
        input: "<p>Fish &am chips</p>",
        covers: "Partial entity sequence without semicolon remains literal text.",
        tags: &["entity", "partial", "text"],
        invariants: &[
            Invariant::PartialEntityRemainsLiteral,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "entity_mixed_with_tags",
        input: "<p>hi &amp; <b>bye</b> &#169;</p>",
        covers: "Mixed text, entities, and tags.",
        tags: &["entity", "mixed", "text", "tags"],
        invariants: &[
            Invariant::DecodesNamedEntities,
            Invariant::DecodesNumericEntities,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "attr_quoted_unquoted",
        input: "<div class=\"a b\" data-x=1>ok</div>",
        covers: "Quoted and unquoted attribute values.",
        tags: &["attribute", "quoted", "unquoted"],
        invariants: &[
            Invariant::AcceptsMixedAttributeSyntax,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "attr_quote_variants",
        input: "<input value='a b' title=\"c d\">",
        covers: "Single and double quoted attribute values.",
        tags: &["attribute", "quoted", "single-quote", "double-quote"],
        invariants: &[
            Invariant::AcceptsMixedAttributeSyntax,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "attr_whitespace_variations",
        input: "<div   id =  \"a\"   class=foo  >ok</div>",
        covers: "Whitespace variations around attributes.",
        tags: &["attribute", "whitespace", "spacing"],
        invariants: &[
            Invariant::AcceptsMixedAttributeSyntax,
            Invariant::AttributesParsedWithSpacing,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "attr_boolean_empty",
        input: "<input disabled required data-empty=\"\">",
        covers: "Boolean and empty attributes.",
        tags: &["attribute", "boolean", "empty"],
        invariants: &[
            Invariant::BooleanAttributePresent,
            Invariant::EmptyAttributeValuePreserved,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "comment_basic",
        input: "<!--split--><p>ok</p>",
        covers: "Comment start/end markers.",
        tags: &["comment", "markers"],
        invariants: &[Invariant::HasCommentToken, Invariant::FullEqualsChunkedDom],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Comment,
    },
    GoldenFixture {
        name: "comment_terminator_edge",
        input: "text<!--x-->tail",
        covers: "Comment terminator boundary with surrounding text.",
        tags: &["comment", "terminator", "text"],
        invariants: &[Invariant::HasCommentToken, Invariant::FullEqualsChunkedDom],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Comment,
    },
    GoldenFixture {
        name: "doctype_mixed_case",
        input: "<!DoCtYpE html><p>ok</p>",
        covers: "Mixed-case doctype token.",
        tags: &["doctype", "case"],
        invariants: &[Invariant::HasDoctypeToken, Invariant::FullEqualsChunkedDom],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Doctype,
    },
    GoldenFixture {
        name: "rawtext_script_many_lt",
        input: "<script>if (a < b && c << 1) {}</script>",
        covers: "Rawtext containing many < characters.",
        tags: &["rawtext", "script", "lt"],
        invariants: &[
            Invariant::ScriptRawtextVerbatim,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::ScriptRawtextVerbatim,
                reason: "rawtext handling is still partial; keep fixture for future parity",
            }],
        },
        kind: FixtureKind::Rawtext,
    },
    GoldenFixture {
        name: "rawtext_close_tag",
        input: "<script>hi</script>",
        covers: "Rawtext close tag present for split tests.",
        tags: &["rawtext", "script", "close-tag"],
        invariants: &[
            Invariant::RawtextCloseTagRecognized,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::RawtextCloseTagRecognized,
                reason: "rawtext close tag splitting may be incomplete",
            }],
        },
        kind: FixtureKind::Rawtext,
    },
    GoldenFixture {
        name: "rawtext_near_match",
        input: "<script>var s = \"</scriptx>\";</script>",
        covers: "Near-match rawtext end tag inside body.",
        tags: &["rawtext", "script", "near-match"],
        invariants: &[
            Invariant::RawtextNearMatchStaysText,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::RawtextNearMatchStaysText,
                reason: "rawtext near-match rules still under development",
            }],
        },
        kind: FixtureKind::Rawtext,
    },
    GoldenFixture {
        name: "tag_custom_element",
        input: "<my-component data-x=1></my-component>",
        covers: "Custom element tag names.",
        tags: &["tag-name", "custom-element"],
        invariants: &[
            Invariant::CustomTagRecognized,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::TagName,
    },
    GoldenFixture {
        name: "tag_namespace",
        input: "<svg:rect width=\"1\" height=\"1\"></svg:rect>",
        covers: "Namespaced tag name with colon.",
        tags: &["tag-name", "namespace", "colon"],
        invariants: &[
            Invariant::NamespacedTagRecognized,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::NamespacedTagRecognized,
                reason: "tag-name charset rules for namespaces are incomplete",
            }],
        },
        kind: FixtureKind::TagName,
    },
];

pub fn fixtures() -> &'static [GoldenFixture] {
    GOLDEN_CORPUS_V1
}

#[cfg(test)]
mod tests {
    use super::{AllowedFailure, GoldenFixture, fixtures};
    use crate::dom_snapshot::{DomSnapshotOptions, compare_dom};
    use crate::test_harness::{ChunkPlan, default_chunk_plans, run_chunked_with_tokens, run_full};
    use crate::{Node, Token, TokenStream};
    use std::collections::HashSet;

    #[test]
    fn golden_corpus_has_metadata() {
        let corpus = fixtures();
        assert!(!corpus.is_empty(), "expected at least one golden fixture");
        let mut names: HashSet<&'static str> = HashSet::new();
        let mut kind_invariants = HashSet::new();
        for &GoldenFixture {
            name,
            input,
            covers,
            tags,
            invariants,
            expectation,
            kind,
        } in corpus
        {
            assert!(!name.trim().is_empty(), "fixture name must be non-empty");
            assert!(!input.trim().is_empty(), "fixture input must be non-empty");
            assert!(
                !covers.trim().is_empty(),
                "fixture covers must be non-empty"
            );
            assert!(!tags.is_empty(), "fixture tags must be non-empty: {name}");
            for &tag in tags {
                assert!(
                    !tag.trim().is_empty(),
                    "fixture tag must be non-empty: {name}"
                );
            }
            assert!(names.insert(name), "fixture name must be unique: {name}");
            assert!(
                !invariants.is_empty(),
                "fixture invariants must be non-empty: {name}"
            );
            let mut inv_set = HashSet::new();
            for inv in invariants.iter().copied() {
                assert!(
                    inv_set.insert(inv),
                    "duplicate invariant on fixture: {name}: {inv}"
                );
            }
            assert!(
                unique_kind_invariants(kind, invariants, tags, &mut kind_invariants),
                "fixture kind+invariants+tags must be unique: {name}"
            );
            validate_allowed(expectation, invariants, name);
        }
    }

    fn unique_kind_invariants(
        kind: super::FixtureKind,
        invariants: &[super::Invariant],
        tags: &[&'static str],
        seen: &mut HashSet<(super::FixtureKind, Vec<super::Invariant>, Vec<&'static str>)>,
    ) -> bool {
        let mut invs = invariants.to_vec();
        invs.sort_unstable();
        let mut tag_list = tags.to_vec();
        tag_list.sort_unstable();
        seen.insert((kind, invs, tag_list))
    }

    fn validate_allowed(
        expectation: super::Expectation,
        invariants: &[super::Invariant],
        name: &str,
    ) {
        if let super::Expectation::AllowedToFail { allowed } = expectation {
            assert!(
                !allowed.is_empty(),
                "fixture allowed-to-fail must declare allowed invariants: {name}"
            );
            for AllowedFailure { invariant, reason } in allowed {
                assert!(
                    !reason.trim().is_empty(),
                    "fixture allowed-to-fail must have a reason: {name}"
                );
                assert!(
                    invariants.contains(invariant),
                    "allowed invariant must be listed on fixture: {name}"
                );
            }
        }
    }

    #[test]
    fn golden_corpus_v1_runs_across_default_chunk_plans() {
        let plans = default_chunk_plans();
        let mut failures = Vec::new();
        for fixture in fixtures() {
            failures.extend(run_golden_fixture(fixture, plans));
        }
        if !failures.is_empty() {
            let report = failures.join("\n");
            panic!("golden corpus failures:\n{report}");
        }
    }

    fn run_golden_fixture(fixture: &GoldenFixture, plans: &[ChunkPlan]) -> Vec<String> {
        let mut failures = Vec::new();
        let strict_xpass = std::env::var("BORROWSER_STRICT_XPASS").is_ok();
        let full_dom = run_full(fixture.input);
        let tags_label = format!("[{}]", fixture.tags.join(","));
        for plan in plans {
            let (chunked_dom, chunked_tokens) = run_chunked_with_tokens(fixture.input, plan);
            for &inv in fixture.invariants {
                let result =
                    check_invariant(fixture, inv, &full_dom, &chunked_dom, &chunked_tokens);
                match result {
                    Ok(()) => {
                        if let Some(reason) = is_allowed_to_fail(fixture, inv) {
                            if strict_xpass {
                                failures.push(format!(
                                    "{} {} :: {:?} :: {} :: XPASS (allowed to fail: {reason})",
                                    fixture.name, tags_label, plan, inv
                                ));
                            } else {
                                eprintln!(
                                    "XPASS: {} {} :: {:?} :: {} :: {reason}",
                                    fixture.name, tags_label, plan, inv
                                );
                            }
                        }
                    }
                    Err(message) => {
                        if is_allowed_to_fail(fixture, inv).is_none() {
                            failures.push(format!(
                                "{} {} :: {:?} :: {} :: {message}",
                                fixture.name, tags_label, plan, inv
                            ));
                        }
                    }
                }
            }
        }
        failures
    }

    fn check_invariant(
        fixture: &GoldenFixture,
        invariant: super::Invariant,
        full_dom: &Node,
        chunked_dom: &Node,
        chunked_tokens: &TokenStream,
    ) -> Result<(), String> {
        match invariant {
            super::Invariant::FullEqualsChunkedDom => {
                compare_dom(full_dom, chunked_dom, DomSnapshotOptions::default())
                    .map_err(|err| err.to_string())
            }
            super::Invariant::HasDoctypeToken => {
                let has_doctype = match chunked_dom {
                    Node::Document { doctype, .. } => doctype.is_some(),
                    _ => false,
                };
                if has_doctype {
                    Ok(())
                } else {
                    Err("expected document doctype".to_string())
                }
            }
            super::Invariant::HasCommentToken => {
                if has_comment(chunked_dom) {
                    Ok(())
                } else {
                    Err("expected comment node".to_string())
                }
            }
            super::Invariant::PreservesUtf8Text => {
                let expected_chars: Vec<char> =
                    fixture.input.chars().filter(|c| !c.is_ascii()).collect();
                if expected_chars.is_empty() {
                    return Ok(());
                }
                let text = collect_text(chunked_dom);
                for ch in expected_chars {
                    if !text.contains(ch) {
                        return Err(format!("expected UTF-8 character {ch} in text"));
                    }
                }
                Ok(())
            }
            super::Invariant::DecodesNamedEntities => {
                let text = collect_text(chunked_dom);
                if fixture.input.contains("&amp;")
                    && (!text.contains('&') || text.contains("&amp;"))
                {
                    return Err("expected named entity decoding for &amp;".to_string());
                }
                Ok(())
            }
            super::Invariant::DecodesNumericEntities => {
                let text = collect_text(chunked_dom);
                if fixture.input.contains("&#123;") && !text.contains('{') {
                    return Err("expected numeric entity &#123; to decode to {".to_string());
                }
                if fixture.input.contains("&#169;") && !text.contains('©') {
                    return Err("expected numeric entity &#169; to decode to ©".to_string());
                }
                if fixture.input.contains("&#x1F600;") && !text.contains('😀') {
                    return Err("expected hex entity &#x1F600; to decode to 😀".to_string());
                }
                Ok(())
            }
            super::Invariant::PartialEntityRemainsLiteral => {
                let text = collect_text(chunked_dom);
                if text.contains("&am") {
                    Ok(())
                } else {
                    Err("expected partial entity &am to remain literal".to_string())
                }
            }
            super::Invariant::AcceptsMixedAttributeSyntax => {
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::AttributesParsedWithSpacing => {
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::BooleanAttributePresent => {
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::EmptyAttributeValuePreserved => {
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::TagBoundariesStable => {
                let expected = expected_tag_names(fixture.input);
                for name in expected {
                    if find_element(chunked_dom, &name).is_none() {
                        return Err(format!("expected element <{name}>"));
                    }
                }
                Ok(())
            }
            super::Invariant::CustomTagRecognized => {
                if find_element(chunked_dom, "my-component").is_some() {
                    Ok(())
                } else {
                    Err("expected <my-component> element".to_string())
                }
            }
            super::Invariant::NamespacedTagRecognized => {
                if find_element(chunked_dom, "svg:rect").is_some() {
                    Ok(())
                } else {
                    Err("expected <svg:rect> element".to_string())
                }
            }
            super::Invariant::ScriptRawtextVerbatim => {
                let expected = script_body_from_input(fixture.input)
                    .ok_or_else(|| "expected <script> body in fixture input".to_string())?;
                let actual = script_text(chunked_dom)
                    .ok_or_else(|| "expected <script> element in DOM".to_string())?;
                if actual == expected {
                    Ok(())
                } else {
                    Err("script text did not match rawtext body".to_string())
                }
            }
            super::Invariant::RawtextCloseTagRecognized => {
                let Some(actual) = script_text(chunked_dom) else {
                    return Err("expected <script> element in DOM".to_string());
                };
                if actual == "hi" {
                    Ok(())
                } else {
                    Err("expected script text to be \"hi\"".to_string())
                }
            }
            super::Invariant::RawtextNearMatchStaysText => {
                let Some(actual) = script_text(chunked_dom) else {
                    return Err("expected <script> element in DOM".to_string());
                };
                if actual.contains("</scriptx>") {
                    Ok(())
                } else {
                    Err("expected rawtext to contain </scriptx>".to_string())
                }
            }
        }
    }

    fn is_allowed_to_fail(
        fixture: &GoldenFixture,
        invariant: super::Invariant,
    ) -> Option<&'static str> {
        match fixture.expectation {
            super::Expectation::MustPass => None,
            super::Expectation::AllowedToFail { allowed } => allowed
                .iter()
                .find(|entry| entry.invariant == invariant)
                .map(|entry| entry.reason),
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
            Node::Element { name: tag, .. } if tag.eq_ignore_ascii_case(name) => Some(node),
            Node::Document { children, .. } | Node::Element { children, .. } => {
                children.iter().find_map(|child| find_element(child, name))
            }
            _ => None,
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
        let start = start + "<script>".len();
        if start > end {
            return None;
        }
        Some(&input[start..end])
    }

    fn expected_tag_names(input: &str) -> Vec<String> {
        let mut tags = Vec::new();
        let bytes = input.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'<' {
                let mut j = i + 1;
                if j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                    let start = j;
                    j += 1;
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric()
                            || bytes[j] == b'-'
                            || bytes[j] == b':')
                    {
                        j += 1;
                    }
                    if let Ok(name) = std::str::from_utf8(&bytes[start..j]) {
                        tags.push(name.to_ascii_lowercase());
                    }
                }
            }
            i += 1;
        }
        tags
    }

    fn check_token_attributes(fixture_name: &str, stream: &TokenStream) -> Result<(), String> {
        type StartTagAttrs<'a> = (&'a str, &'a [(crate::AtomId, Option<String>)]);
        let atoms = stream.atoms();
        let start_tags: Vec<StartTagAttrs<'_>> = stream
            .tokens()
            .iter()
            .filter_map(|token| {
                if let Token::StartTag {
                    name, attributes, ..
                } = token
                {
                    Some((atoms.resolve(*name), attributes.as_slice()))
                } else {
                    None
                }
            })
            .collect();
        match fixture_name {
            "attr_quoted_unquoted" => {
                let (_, attrs) = start_tags
                    .iter()
                    .find(|(tag, _)| *tag == "div")
                    .ok_or_else(|| "expected <div> start tag".to_string())?;
                let class = find_attr(atoms, attrs, "class");
                let data_x = find_attr(atoms, attrs, "data-x");
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
                let value = find_attr(atoms, attrs, "value");
                let title = find_attr(atoms, attrs, "title");
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
                let id = find_attr(atoms, attrs, "id");
                let class = find_attr(atoms, attrs, "class");
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
                let data_empty = find_attr(atoms, attrs, "data-empty");
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
        atoms: &'a crate::AtomTable,
        attrs: &'a [(crate::AtomId, Option<String>)],
        name: &str,
    ) -> Option<&'a str> {
        attrs.iter().find_map(|(key, value)| {
            if atoms.resolve(*key).eq_ignore_ascii_case(name) {
                Some(value.as_deref().unwrap_or(""))
            } else {
                None
            }
        })
    }

    fn has_attr(
        atoms: &crate::AtomTable,
        attrs: &[(crate::AtomId, Option<String>)],
        name: &str,
    ) -> bool {
        attrs
            .iter()
            .any(|(key, _)| atoms.resolve(*key).eq_ignore_ascii_case(name))
    }
}
