use super::model::{AllowedFailure, Expectation, FixtureKind, GoldenFixture, Invariant};

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
