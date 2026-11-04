//! Authored CSS resource observations, independent of cascade participation.
//! CSS owns syntax/decoding and reference meaning. Consumers supply source/base
//! provenance and delegate URL semantics to addressing. No I/O or retained cache.
use crate::model::{self, AtRuleBlock, ValueComponent as V, ValueToken as T};
use crate::syntax::{
    CssInput, CssSpan, DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats,
    SyntaxDiagnostic, escape::decode,
};
use crate::{CascadeDeclarationApplicability as A, CssUrlValue, SelectorMatchability};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssResourceRole {
    Image,
    Font,
    StylesheetImport,
    Cursor,
    Mask,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssResourceContext {
    Import,
    FontFace,
    ImageSet,
    CustomProperty,
    Substitution,
    UnsupportedFunction,
    UnsupportedRule,
    UrlModifier,
}

/// Shape and blocking contexts are orthogonal. This enum does not grant
/// property acceptance, effective participation or permission to issue requests.
pub enum CssReferenceDisposition {
    Literal(CssUrlValue),
    Empty,
    LocalFragment(CssUrlValue),
    InvalidCss,
}
impl CssReferenceDisposition {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Literal(_) => "Literal",
            Self::Empty => "Empty",
            Self::LocalFragment(_) => "LocalFragment",
            Self::InvalidCss => "InvalidCss",
        }
    }
}
pub struct CssResourceReference {
    pub span: CssSpan,
    pub role: CssResourceRole,
    pub disposition: CssReferenceDisposition,
    pub contexts: Vec<CssResourceContext>,
    pub declaration_applicability: Option<A>,
    pub selector_matchability: Option<SelectorMatchability>,
}
/// Input ownership keeps every source-bound span valid. Neither this report nor
/// its authored values implement Debug: production formatting must redact them.
pub struct CssResourceDiscovery<'a> {
    pub input: Cow<'a, CssInput>,
    pub references: Vec<CssResourceReference>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub feature_notices: Vec<(CssSpan, CssResourceContext)>,
    /// Optional explanatory output was omitted; retained source coverage is
    /// still described solely by `stats.hit_limit`.
    pub feature_notices_truncated: bool,
    pub stats: ParseStats,
}

pub fn discover_stylesheet_resource_references<'a>(
    parse: &'a model::StylesheetParse,
    options: &ParseOptions,
) -> CssResourceDiscovery<'a> {
    let mut collector = Collector::new(
        Cow::Borrowed(&parse.input),
        parse.diagnostics.clone(),
        parse.stats.clone(),
        options,
    );
    for rule in &parse.stylesheet.rules {
        match rule {
            model::Rule::Style(rule) => {
                for declaration in &rule.declarations.declarations {
                    collector.declaration(declaration, Some(rule.selectors.matchability()));
                }
            }
            model::Rule::At(rule) => {
                let name = rule
                    .name
                    .as_deref()
                    .and_then(|s| decode(s, false))
                    .map(|s| s.to_ascii_lowercase());
                let (context, role) = match name.as_deref() {
                    Some("import") => (
                        CssResourceContext::Import,
                        CssResourceRole::StylesheetImport,
                    ),
                    Some("font-face") => (CssResourceContext::FontFace, CssResourceRole::Font),
                    _ => (
                        CssResourceContext::UnsupportedRule,
                        CssResourceRole::Unknown,
                    ),
                };
                collector.notice(rule.span, context);
                let prelude =
                    model::build_value_components(&collector.report.input, &rule.prelude.values);
                if context == CssResourceContext::Import
                    && let Some(V::Token(T::String { span, text })) =
                        prelude.iter().find(|v| !trivia(v))
                {
                    collector.literal(
                        *span,
                        text.text.as_deref(),
                        true,
                        ReferenceSite {
                            role,
                            contexts: vec![context],
                            acceptance: None,
                            selector: None,
                        },
                    );
                }
                if context == CssResourceContext::Import
                    && let Some(V::Token(T::BadString { span })) =
                        prelude.iter().find(|v| !trivia(v))
                {
                    // Only the URL-bearing leading import position gives a
                    // BadString resource meaning; ordinary strings stay inert.
                    collector.invalid(*span, role, vec![context], None, None);
                }
                collector.walk(&prelude, role, &[context], None, None);
                if let Some(AtRuleBlock::Preserved(block)) = &rule.block {
                    let components =
                        model::build_value_components(&collector.report.input, &block.values);
                    collector.walk(&components, role, &[context], None, None);
                }
            }
        }
    }
    collector.report
}

pub fn discover_style_attribute_resource_references(
    text: &str,
    options: &ParseOptions,
) -> CssResourceDiscovery<'static> {
    let parse = model::parse_declaration_list_with_options(text, options);
    let mut collector = Collector::new(
        Cow::Owned(parse.input),
        parse.diagnostics,
        parse.stats,
        options,
    );
    for declaration in &parse.declarations {
        collector.declaration(declaration, None);
    }
    collector.report
}

fn trivia(value: &V) -> bool {
    matches!(value, V::Token(T::Whitespace { .. } | T::Comment { .. }))
}
fn function_name(value: &crate::ValueFunction) -> Option<String> {
    decode(value.name.text.as_deref()?, false).map(|s| s.to_ascii_lowercase())
}
fn has_substitution(components: &[V]) -> bool {
    let mut stack: Vec<_> = components.iter().collect();
    while let Some(value) = stack.pop() {
        match value {
            V::Function(f) => {
                if matches!(function_name(f).as_deref(), Some("var" | "env" | "attr")) {
                    return true;
                }
                stack.extend(f.components.iter());
            }
            V::SimpleBlock(b) => stack.extend(b.components.iter()),
            _ => {}
        }
    }
    false
}
fn role(name: Option<&str>) -> CssResourceRole {
    match name {
        Some(
            "background"
            | "background-image"
            | "border-image"
            | "border-image-source"
            | "list-style"
            | "list-style-image"
            | "content",
        ) => CssResourceRole::Image,
        Some("cursor") => CssResourceRole::Cursor,
        Some("mask" | "mask-image") => CssResourceRole::Mask,
        _ => CssResourceRole::Unknown,
    }
}
// Reuse the production registry/parsers, rather than infer property acceptance
// from discovery. Escaped discovery names never rewrite the cascade model.
fn applicability(declaration: &model::Declaration) -> A {
    match declaration.name.kind {
        model::PropertyNameKind::Custom => A::CustomProperty,
        model::PropertyNameKind::Invalid => A::InvalidPropertyName,
        model::PropertyNameKind::Standard => {
            let Some(name) = declaration.name.text.as_deref() else {
                return A::InvalidPropertyName;
            };
            if let Some(property) = crate::property_registry().lookup_id(name) {
                if crate::parse_specified_declaration_value(property, &declaration.value).is_ok() {
                    A::Supported(property)
                } else {
                    A::InvalidValue(property)
                }
            } else if let Some(shorthand) = crate::shorthand_registry().lookup_id(name) {
                // URL-bearing inputs are rejected by the existing shorthand
                // grammar. Validate through that grammar rather than a table.
                match crate::expand_shorthand_declaration(shorthand, &declaration.value) {
                    Ok(expansion) => expansion
                        .longhands()
                        .first()
                        .map_or(A::InvalidShorthandValue(shorthand), |p| {
                            A::Supported(p.property())
                        }),
                    Err(_) => A::InvalidShorthandValue(shorthand),
                }
            } else {
                A::UnsupportedProperty
            }
        }
    }
}
struct ReferenceSite {
    role: CssResourceRole,
    contexts: Vec<CssResourceContext>,
    acceptance: Option<A>,
    selector: Option<SelectorMatchability>,
}

struct Collector<'a, 'o> {
    report: CssResourceDiscovery<'a>,
    options: &'o ParseOptions,
}
impl<'a, 'o> Collector<'a, 'o> {
    fn new(
        input: Cow<'a, CssInput>,
        diagnostics: Vec<SyntaxDiagnostic>,
        stats: ParseStats,
        options: &'o ParseOptions,
    ) -> Self {
        Self {
            report: CssResourceDiscovery {
                input,
                references: Vec::new(),
                diagnostics,
                feature_notices: Vec::new(),
                feature_notices_truncated: false,
                stats,
            },
            options,
        }
    }
    fn notice(&mut self, span: CssSpan, context: CssResourceContext) {
        if self.report.feature_notices.len() < self.options.limits.max_diagnostics {
            self.report.feature_notices.push((span, context));
        } else {
            self.report.feature_notices_truncated = true;
        }
    }
    fn declaration(
        &mut self,
        declaration: &model::Declaration,
        selector: Option<SelectorMatchability>,
    ) {
        let name = declaration
            .name
            .text
            .as_deref()
            .and_then(|s| decode(s, false))
            .map(|s| s.to_ascii_lowercase());
        let mut contexts = Vec::new();
        if name.as_deref().is_some_and(|s| s.starts_with("--")) {
            contexts.push(CssResourceContext::CustomProperty);
            self.notice(declaration.span, CssResourceContext::CustomProperty);
        }
        if has_substitution(&declaration.value.components) {
            contexts.push(CssResourceContext::Substitution);
            self.notice(declaration.span, CssResourceContext::Substitution);
        }
        self.walk(
            &declaration.value.components,
            role(name.as_deref()),
            &contexts,
            Some(applicability(declaration)),
            selector,
        );
    }
    fn literal(&mut self, span: CssSpan, text: Option<&str>, string: bool, site: ReferenceSite) {
        let ReferenceSite {
            role,
            contexts,
            acceptance,
            selector,
        } = site;
        let disposition = match text.and_then(|s| decode(s, string)) {
            Some(value) if value.is_empty() => CssReferenceDisposition::Empty,
            Some(value) if value.starts_with('#') => {
                CssReferenceDisposition::LocalFragment(CssUrlValue::new(span, value))
            }
            Some(value) => CssReferenceDisposition::Literal(CssUrlValue::new(span, value)),
            None => CssReferenceDisposition::InvalidCss,
        };
        self.report.references.push(CssResourceReference {
            span,
            role,
            disposition,
            contexts,
            declaration_applicability: acceptance,
            selector_matchability: selector,
        });
    }
    fn invalid(
        &mut self,
        span: CssSpan,
        role: CssResourceRole,
        contexts: Vec<CssResourceContext>,
        acceptance: Option<A>,
        selector: Option<SelectorMatchability>,
    ) {
        self.literal(
            span,
            None,
            false,
            ReferenceSite {
                role,
                contexts,
                acceptance,
                selector,
            },
        );
        if self.options.collect_diagnostics
            && self.report.diagnostics.len() < self.options.limits.max_diagnostics
            && !self
                .report
                .diagnostics
                .iter()
                .any(|d| d.kind == DiagnosticKind::BadUrl && d.byte_offset == span.start)
        {
            self.report.stats.diagnostics_emitted =
                self.report.stats.diagnostics_emitted.saturating_add(1);
            self.report.diagnostics.push(SyntaxDiagnostic {
                severity: DiagnosticSeverity::Warning,
                kind: DiagnosticKind::BadUrl,
                byte_offset: span.start,
                message: "invalid resource URL value".into(),
            });
        }
    }
    fn walk(
        &mut self,
        values: &[V],
        role: CssResourceRole,
        contexts: &[CssResourceContext],
        acceptance: Option<A>,
        selector: Option<SelectorMatchability>,
    ) {
        // Iterative traversal stays bounded by the parser's retained component
        // tree. Context stacks describe enclosing constructs, not dependencies.
        let mut stack: Vec<_> = values
            .iter()
            .rev()
            .map(|v| (v, contexts.to_vec()))
            .collect();
        while let Some((value, mut contexts)) = stack.pop() {
            match value {
                V::Token(T::Url { span, text }) => self.literal(
                    *span,
                    text.text.as_deref(),
                    false,
                    ReferenceSite {
                        role,
                        contexts,
                        acceptance,
                        selector,
                    },
                ),
                V::Token(T::BadUrl { span }) => {
                    self.invalid(*span, role, contexts, acceptance, selector)
                }
                V::Function(f) => {
                    let name = function_name(f);
                    if name.as_deref() == Some("url") {
                        let args: Vec<_> = f.components.iter().filter(|v| !trivia(v)).collect();
                        match args.as_slice() {
                            [V::Token(T::String { text, .. })] => self.literal(
                                f.span,
                                text.text.as_deref(),
                                true,
                                ReferenceSite {
                                    role,
                                    contexts,
                                    acceptance,
                                    selector,
                                },
                            ),
                            [V::Token(T::String { text, .. }), rest @ ..]
                                if rest.iter().all(|v| {
                                    matches!(v, V::Token(T::Ident { .. }) | V::Function(_))
                                }) =>
                            {
                                contexts.push(CssResourceContext::UrlModifier);
                                self.notice(f.span, CssResourceContext::UrlModifier);
                                self.literal(
                                    f.span,
                                    text.text.as_deref(),
                                    true,
                                    ReferenceSite {
                                        role,
                                        contexts,
                                        acceptance,
                                        selector,
                                    },
                                );
                            }
                            _ => self.invalid(f.span, role, contexts, acceptance, selector),
                        }
                    } else {
                        let context = match name.as_deref() {
                            Some("image-set" | "-webkit-image-set") => CssResourceContext::ImageSet,
                            Some("var" | "env" | "attr") => CssResourceContext::Substitution,
                            _ => CssResourceContext::UnsupportedFunction,
                        };
                        // Non-URL gradients are unrelated to resource addressing.
                        if context != CssResourceContext::UnsupportedFunction
                            || contains_url(&f.components)
                        {
                            self.notice(f.span, context);
                        }
                        contexts.push(context);
                        stack.extend(f.components.iter().rev().map(|v| (v, contexts.clone())));
                    }
                }
                V::SimpleBlock(b) => {
                    contexts.push(CssResourceContext::UnsupportedRule);
                    if contains_url(&b.components) {
                        self.notice(b.span, CssResourceContext::UnsupportedRule);
                    }
                    stack.extend(b.components.iter().rev().map(|v| (v, contexts.clone())));
                }
                _ => {} // Ordinary strings, bad strings and comments are not URLs.
            }
        }
    }
}
fn contains_url(values: &[V]) -> bool {
    let mut stack: Vec<_> = values.iter().collect();
    while let Some(value) = stack.pop() {
        match value {
            V::Token(T::Url { .. } | T::BadUrl { .. }) => return true,
            V::Function(f) => {
                if function_name(f).as_deref() == Some("url") {
                    return true;
                }
                stack.extend(f.components.iter());
            }
            V::SimpleBlock(b) => stack.extend(b.components.iter()),
            _ => {}
        }
    }
    false
}
#[cfg(test)]
mod tests;
