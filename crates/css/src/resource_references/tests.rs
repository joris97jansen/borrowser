use super::*;
fn sheet(text: &str) -> model::StylesheetParse {
    crate::parse_stylesheet_with_options(text, &ParseOptions::stylesheet())
}
fn value(reference: &CssResourceReference) -> &str {
    match &reference.disposition {
        CssReferenceDisposition::Literal(v) | CssReferenceDisposition::LocalFragment(v) => {
            v.value()
        }
        _ => "",
    }
}
#[test]
fn resource_reference_notice_budget_does_not_change_retained_source_coverage() {
    let parsed = sheet("p{--accent:red;background:url(a.png),url(a.png);--asset:url(#m)}");
    let original_stats = parsed.stats.clone();
    assert!(!original_stats.hit_limit);
    let adequate = discover_stylesheet_resource_references(&parsed, &ParseOptions::stylesheet());
    let mut options = ParseOptions::stylesheet();
    options.limits.max_diagnostics = 0;
    let exhausted = discover_stylesheet_resource_references(&parsed, &options);

    assert_eq!(parsed.stats, original_stats);
    assert_eq!(adequate.stats, original_stats);
    assert_eq!(exhausted.stats, original_stats);
    assert_eq!(adequate.feature_notices.len(), 2);
    assert!(!adequate.feature_notices_truncated);
    assert!(exhausted.feature_notices.is_empty());
    assert!(exhausted.feature_notices_truncated);
    assert_eq!(adequate.references.len(), 3);
    for (before, after) in adequate.references.iter().zip(&exhausted.references) {
        assert_eq!(before.span, after.span);
        assert_eq!(before.role, after.role);
        assert_eq!(before.disposition.label(), after.disposition.label());
        assert_eq!(value(before), value(after));
        assert_eq!(before.contexts, after.contexts);
        assert_eq!(
            before.declaration_applicability,
            after.declaration_applicability
        );
        assert_eq!(before.selector_matchability, after.selector_matchability);
    }
    assert_eq!(exhausted.references.len(), adequate.references.len());
    assert_eq!(
        exhausted.references[2].contexts,
        [CssResourceContext::CustomProperty]
    );
    assert_eq!(
        exhausted.references.iter().map(value).collect::<Vec<_>>(),
        ["a.png", "a.png", "#m"]
    );
    assert!(
        exhausted
            .references
            .windows(2)
            .all(|w| w[0].span.start < w[1].span.start)
    );
}

#[test]
fn resource_reference_added_diagnostic_cap_does_not_truncate_source() {
    // The syntax parse is complete; URL-function argument validation adds the
    // explanatory BadUrl diagnostic only during discovery.
    let parsed = sheet("p{width:url('a','b');background:url(a.png)}");
    let original_stats = parsed.stats.clone();
    assert!(!original_stats.hit_limit);
    assert!(parsed.diagnostics.is_empty());
    let adequate = discover_stylesheet_resource_references(&parsed, &ParseOptions::stylesheet());
    let mut options = ParseOptions::stylesheet();
    options.limits.max_diagnostics = 0;
    let exhausted = discover_stylesheet_resource_references(&parsed, &options);
    assert_eq!(parsed.stats, original_stats);
    assert!(!adequate.stats.hit_limit);
    assert!(!exhausted.stats.hit_limit);
    assert_eq!(adequate.diagnostics.len(), 1);
    assert_eq!(adequate.diagnostics[0].kind, DiagnosticKind::BadUrl);
    assert!(exhausted.diagnostics.is_empty());
    for report in [&adequate, &exhausted] {
        assert_eq!(report.references.len(), 2);
        assert_eq!(report.references[0].disposition.label(), "InvalidCss");
        assert_eq!(report.references[1].disposition.label(), "Literal");
        assert_eq!(value(&report.references[1]), "a.png");
    }
}

#[test]
fn resource_reference_shapes_contexts_and_decoding_are_independent() {
    let parsed = sheet(
        r##"p { background-image: url(a.png), url('b.png'), u\72 l("c\2e png"), url(a.png); --asset: url("#mask"); cursor: url(""); mask-image: url(#mask); content: "url(no.png)"; } @import url(""); @font-face {src:url(font.woff2)}"##,
    );
    let report = discover_stylesheet_resource_references(&parsed, &ParseOptions::stylesheet());
    let refs = &report.references;
    assert_eq!(refs.len(), 9);
    assert_eq!(
        refs.iter().take(4).map(value).collect::<Vec<_>>(),
        ["a.png", "b.png", "c.png", "a.png"]
    );
    assert!(matches!(
        refs[4].disposition,
        CssReferenceDisposition::LocalFragment(_)
    ));
    assert_eq!(refs[4].contexts, [CssResourceContext::CustomProperty]);
    assert!(matches!(
        refs[5].disposition,
        CssReferenceDisposition::Empty
    ));
    assert_eq!(refs[5].role, CssResourceRole::Cursor);
    assert!(matches!(
        refs[6].disposition,
        CssReferenceDisposition::LocalFragment(_)
    ));
    assert_eq!(refs[6].role, CssResourceRole::Mask);
    assert!(matches!(
        refs[7].disposition,
        CssReferenceDisposition::Empty
    ));
    assert_eq!(refs[7].contexts, [CssResourceContext::Import]);
    assert_eq!(refs[8].role, CssResourceRole::Font);
    assert_eq!(refs[8].contexts, [CssResourceContext::FontFace]);
    assert!(refs.windows(2).all(|w| w[0].span.start < w[1].span.start));
}
#[test]
fn resource_reference_unsupported_and_substitution_contexts_never_disappear() {
    let parsed = sheet(
        "p {background-image: image-set(url(a.png) 1x, 'b.png' 2x); --x: weird(url(#mask)); mask:var(--m,url(fallback.svg)); width:url(bad\"x); } @unknown { p { background:url(nested.png) }}",
    );
    let report = discover_stylesheet_resource_references(&parsed, &ParseOptions::stylesheet());
    assert_eq!(report.references.len(), 5);
    assert_eq!(
        report.references[0].contexts,
        [CssResourceContext::ImageSet]
    );
    assert_eq!(
        report.references[1].contexts,
        [
            CssResourceContext::CustomProperty,
            CssResourceContext::UnsupportedFunction
        ]
    );
    assert!(
        report.references[2]
            .contexts
            .contains(&CssResourceContext::Substitution)
    );
    assert!(matches!(
        report.references[3].disposition,
        CssReferenceDisposition::InvalidCss
    ));
    assert!(
        report.references[4]
            .contexts
            .contains(&CssResourceContext::UnsupportedRule)
    );
    let gradient = sheet("p { background:linear-gradient(red,blue);content:'url(fake)'; }");
    assert!(
        discover_stylesheet_resource_references(&gradient, &ParseOptions::stylesheet())
            .references
            .is_empty()
    );
}
#[test]
fn resource_reference_eof_and_token_context_recovery_preserve_payloads() {
    for text in [
        "background:url(icon.svg",
        "background:url(\"icon.svg\"",
        "background:url(\"icon.svg",
    ] {
        let report =
            discover_style_attribute_resource_references(text, &ParseOptions::style_attribute());
        assert_eq!(report.references.len(), 1, "{text}");
        assert_eq!(value(&report.references[0]), "icon.svg");
        assert!(!report.diagnostics.is_empty());
    }
    let unrelated = discover_style_attribute_resource_references(
        "content:'bad\n;",
        &ParseOptions::style_attribute(),
    );
    assert!(unrelated.references.is_empty());
    assert!(
        unrelated
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::UnterminatedString)
    );
    let report = discover_style_attribute_resource_references(
        "background:url('a\\\nb.svg'); cursor:url(a\\\nb.svg)",
        &ParseOptions::style_attribute(),
    );
    assert_eq!(report.references.len(), 2);
    assert_eq!(value(&report.references[0]), "ab.svg");
    assert!(matches!(
        report.references[1].disposition,
        CssReferenceDisposition::InvalidCss
    ));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::BadUrl)
    );
}
#[test]
fn resource_reference_limits_and_property_acceptance_are_preserved() {
    let mut options = ParseOptions::stylesheet();
    options.limits.max_lexical_tokens = 12;
    let parsed = crate::parse_stylesheet_with_options(
        "p { background:url(a); --x:url(#m); color:red; }",
        &options,
    );
    let report = discover_stylesheet_resource_references(&parsed, &options);
    assert!(report.stats.hit_limit);
    assert!(!report.references.is_empty());
    for property in crate::PropertyId::ALL {
        let report = discover_style_attribute_resource_references(
            &format!("{}:url(a)", property.name()),
            &ParseOptions::style_attribute(),
        );
        assert_eq!(
            report.references[0].declaration_applicability,
            Some(A::InvalidValue(property))
        );
    }
    let report = discover_style_attribute_resource_references(
        "outline:url(a)",
        &ParseOptions::style_attribute(),
    );
    assert!(matches!(
        report.references[0].declaration_applicability,
        Some(A::InvalidShorthandValue(_))
    ));
}

#[test]
fn resource_reference_escape_contexts_and_invalid_shapes_are_explicit() {
    let parsed = sheet(
        r##"p{background\2d image:U\000052 L(a\2e png); mask:url(\23 mask);cursor:url(open\(paren.png);content:url("\0 \d800 \110000 ");width:url("a","b")} @\69 mport "a\2e css";"##,
    );
    let report = discover_stylesheet_resource_references(&parsed, &ParseOptions::stylesheet());
    assert_eq!(report.references.len(), 6);
    assert_eq!(value(&report.references[0]), "a.png");
    assert_eq!(report.references[0].role, CssResourceRole::Image);
    assert_eq!(value(&report.references[1]), "#mask");
    assert_eq!(value(&report.references[2]), "open(paren.png");
    assert_eq!(value(&report.references[3]), "\u{fffd}\u{fffd}\u{fffd}");
    assert!(matches!(
        report.references[4].disposition,
        CssReferenceDisposition::InvalidCss
    ));
    assert_eq!(value(&report.references[5]), "a.css");
    assert_eq!(report.references[5].contexts, [CssResourceContext::Import]);
}

#[test]
fn resource_reference_bad_string_is_contextual_to_import_or_url() {
    let parsed = sheet("@import 'bad\n; p{content:'bad\n; background:url('bad\n)}");
    let report = discover_stylesheet_resource_references(&parsed, &ParseOptions::stylesheet());
    assert_eq!(report.references.len(), 2);
    assert!(
        report
            .references
            .iter()
            .all(|r| matches!(r.disposition, CssReferenceDisposition::InvalidCss))
    );
    assert_eq!(report.references[0].contexts, [CssResourceContext::Import]);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|d| d.kind == DiagnosticKind::UnterminatedString)
            .count(),
        3
    );
}
