use super::{
    AttributeMatcher, AttributeSelector, AttributeValue, Combinator, ComplexSelector,
    CompoundSelector, SelectorList, SelectorListParseResult, Specificity, SubclassSelector,
    TypeSelector,
};
use crate::syntax::CssSpan;
use std::fmt::Write;

const SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorSnapshotSerializationError {
    LimitExceeded { maximum: usize, observed: usize },
    ReservationFailure { requested: usize },
    FormattingInvariant,
}

struct BoundedSnapshotWriter {
    output: String,
    maximum: usize,
    failure: Option<SelectorSnapshotSerializationError>,
    #[cfg(test)]
    force_reservation_failure: bool,
}

impl BoundedSnapshotWriter {
    fn new(maximum: usize) -> Self {
        Self {
            output: String::new(),
            maximum,
            failure: None,
            #[cfg(test)]
            force_reservation_failure: false,
        }
    }

    fn finish(self) -> Result<String, SelectorSnapshotSerializationError> {
        match self.failure {
            Some(error) => Err(error),
            None => Ok(self.output),
        }
    }
}

impl Write for BoundedSnapshotWriter {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self.failure.is_some() {
            return Err(std::fmt::Error);
        }
        let Some(observed) = self.output.len().checked_add(value.len()) else {
            self.failure = Some(SelectorSnapshotSerializationError::LimitExceeded {
                maximum: self.maximum,
                observed: usize::MAX,
            });
            return Err(std::fmt::Error);
        };
        if observed > self.maximum {
            self.failure = Some(SelectorSnapshotSerializationError::LimitExceeded {
                maximum: self.maximum,
                observed,
            });
            return Err(std::fmt::Error);
        }
        #[cfg(test)]
        if self.force_reservation_failure {
            self.failure = Some(SelectorSnapshotSerializationError::ReservationFailure {
                requested: value.len(),
            });
            return Err(std::fmt::Error);
        }
        if self.output.try_reserve(value.len()).is_err() {
            self.failure = Some(SelectorSnapshotSerializationError::ReservationFailure {
                requested: value.len(),
            });
            return Err(std::fmt::Error);
        }
        self.output.push_str(value);
        Ok(())
    }
}

pub fn serialize_selector_list_for_snapshot(list: &SelectorList) -> String {
    let mut out = String::new();
    write_snapshot_header(&mut out, "selector-list").expect("write selector snapshot header");
    write_selector_list_snapshot_body(&mut out, list, 0).expect("write selector snapshot body");
    out
}

pub fn serialize_selector_parse_result_for_snapshot(result: &SelectorListParseResult) -> String {
    let mut out = String::new();
    write_snapshot_header(&mut out, "selector-parse").expect("write selector snapshot header");
    write_selector_parse_result_snapshot_body(&mut out, result, 0)
        .expect("write selector snapshot body");
    out
}

/// Serializes the canonical selector parse snapshot while enforcing the byte
/// bound during materialization. No partial snapshot is returned on failure.
pub fn serialize_selector_parse_result_for_snapshot_bounded(
    result: &SelectorListParseResult,
    maximum_bytes: usize,
) -> Result<String, SelectorSnapshotSerializationError> {
    let mut out = BoundedSnapshotWriter::new(maximum_bytes);
    if write_snapshot_header(&mut out, "selector-parse").is_ok() {
        let _ = write_selector_parse_result_snapshot_body(&mut out, result, 0);
    }
    out.finish()
}

pub(crate) fn write_selector_list_snapshot_body(
    out: &mut impl Write,
    list: &SelectorList,
    indent: usize,
) -> std::fmt::Result {
    write_indent(out, indent)?;
    write!(out, "span: ")?;
    write_span_label(out, list.span())?;
    writeln!(out)?;
    for (selector_index, selector) in list.selectors().iter().enumerate() {
        write_selector(out, selector, selector_index, indent)?;
    }
    Ok(())
}

pub(crate) fn write_selector_parse_result_snapshot_body(
    out: &mut impl Write,
    result: &SelectorListParseResult,
    indent: usize,
) -> std::fmt::Result {
    match result {
        SelectorListParseResult::Parsed(list) => {
            write_indent(out, indent)?;
            writeln!(out, "result: parsed")?;
            write_selector_list_snapshot_body(out, list, indent)?;
        }
        SelectorListParseResult::Unsupported(list) => {
            write_indent(out, indent)?;
            writeln!(out, "result: unsupported")?;
            write_indent(out, indent)?;
            write!(out, "span: ")?;
            write_span_label(out, list.span())?;
            writeln!(out)?;
            for (feature_index, feature) in list.features().iter().enumerate() {
                write_indent(out, indent)?;
                writeln!(out, "feature[{feature_index}]: {}", feature.stable_label())?;
            }
        }
        SelectorListParseResult::Invalid(list) => {
            write_indent(out, indent)?;
            writeln!(out, "result: invalid")?;
            write_indent(out, indent)?;
            write!(out, "span: ")?;
            write_span_label(out, list.span())?;
            writeln!(out)?;
            write_indent(out, indent)?;
            writeln!(out, "reason: {}", list.reason().stable_label())?;
        }
    }
    Ok(())
}

fn write_snapshot_header(out: &mut impl Write, kind: &str) -> std::fmt::Result {
    writeln!(out, "version: {SNAPSHOT_VERSION}")?;
    writeln!(out, "{kind}")
}

fn write_selector(
    out: &mut impl Write,
    selector: &ComplexSelector,
    index: usize,
    indent: usize,
) -> std::fmt::Result {
    let selector_span = selector.span();
    write_indent(out, indent)?;
    write!(
        out,
        "selector[{index}] @{}..{} specificity=",
        selector_span.start, selector_span.end
    )?;
    write_specificity(out, selector.specificity())?;
    writeln!(out)?;

    write_compound(out, selector.head(), Some(0), indent + 2)?;

    for (combined_index, combined) in selector.tail().iter().enumerate() {
        let combined_span = combined.span();
        write_indent(out, indent + 2)?;
        writeln!(
            out,
            "combined[{combined_index}] {} @{}..{}",
            combinator_label(combined.combinator()),
            combined_span.start,
            combined_span.end
        )?;
        write_compound(out, combined.selector(), None, indent + 4)?;
    }
    Ok(())
}

fn write_compound(
    out: &mut impl Write,
    selector: &CompoundSelector,
    index: Option<usize>,
    indent: usize,
) -> std::fmt::Result {
    let selector_span = selector.span();
    write_indent(out, indent)?;
    match index {
        Some(index) => write!(
            out,
            "compound[{index}] @{}..{} specificity=",
            selector_span.start, selector_span.end
        )?,
        None => write!(
            out,
            "compound @{}..{} specificity=",
            selector_span.start, selector_span.end
        )?,
    };
    write_specificity(out, selector.specificity())?;
    writeln!(out)?;

    if let Some(type_selector) = selector.type_selector() {
        write_indent(out, indent + 2)?;
        write!(out, "- ")?;
        write_type_selector_snapshot(out, type_selector)?;
        writeln!(out)?;
    }

    for subclass in selector.subclasses() {
        write_indent(out, indent + 2)?;
        write!(out, "- ")?;
        write_subclass_selector_snapshot(out, subclass)?;
        writeln!(out)?;
    }
    Ok(())
}

fn write_type_selector_snapshot(out: &mut impl Write, selector: &TypeSelector) -> std::fmt::Result {
    match selector {
        TypeSelector::Universal(selector) => {
            write!(out, "universal(*) node=")?;
            write_span_label(out, Some(selector.span()))
        }
        TypeSelector::Named(selector) => {
            write!(out, "type(")?;
            write_quoted(out, selector.name().text())?;
            write!(out, ") node=")?;
            write_span_label(out, Some(selector.span()))?;
            write!(out, " name=")?;
            write_span_label(out, selector.name().span())
        }
    }
}

fn write_subclass_selector_snapshot(
    out: &mut impl Write,
    selector: &SubclassSelector,
) -> std::fmt::Result {
    match selector {
        SubclassSelector::Id(selector) => {
            write!(out, "id(")?;
            write_quoted(out, selector.name().text())?;
            write!(out, ") node=")?;
            write_span_label(out, Some(selector.span()))?;
            write!(out, " name=")?;
            write_span_label(out, selector.name().span())
        }
        SubclassSelector::Class(selector) => {
            write!(out, "class(")?;
            write_quoted(out, selector.name().text())?;
            write!(out, ") node=")?;
            write_span_label(out, Some(selector.span()))?;
            write!(out, " name=")?;
            write_span_label(out, selector.name().span())
        }
        SubclassSelector::Attribute(selector) => write_attribute_selector_snapshot(out, selector),
        SubclassSelector::TreeStructuralPseudoClass(selector) => {
            write!(
                out,
                "tree-structural-pseudo-class({}) node=",
                selector.pseudo_class().css_keyword()
            )?;
            write_span_label(out, Some(selector.span()))
        }
    }
}

fn write_attribute_selector_snapshot(
    out: &mut impl Write,
    selector: &AttributeSelector,
) -> std::fmt::Result {
    match selector {
        AttributeSelector::Exists(selector) => {
            write!(out, "attribute-exists(name=")?;
            write_quoted(out, selector.name().text())?;
            write!(out, ", name_span=")?;
            write_span_label(out, selector.name().span())?;
            write!(out, ") node=")?;
            write_span_label(out, Some(selector.span()))
        }
        AttributeSelector::Match(selector) => {
            write!(out, "attribute-match(name=")?;
            write_quoted(out, selector.name().text())?;
            write!(out, ", name_span=")?;
            write_span_label(out, selector.name().span())?;
            write!(
                out,
                ", matcher={}, value=",
                attribute_matcher_label(selector.matcher())
            )?;
            write_attribute_value_snapshot(out, selector.value())?;
            write!(out, ") node=")?;
            write_span_label(out, Some(selector.span()))
        }
    }
}

fn write_attribute_value_snapshot(
    out: &mut impl Write,
    value: &AttributeValue,
) -> std::fmt::Result {
    match value {
        AttributeValue::Ident(value) => {
            write!(out, "ident(")?;
            write_quoted(out, value.text())?;
            write!(out, ", span=")?;
            write_span_label(out, value.span())?;
            write!(out, ")")
        }
        AttributeValue::String(value) => {
            write!(out, "string(")?;
            write_quoted(out, value.value())?;
            write!(out, ", span=")?;
            write_span_label(out, value.span())?;
            write!(out, ")")
        }
    }
}

fn write_specificity(out: &mut impl Write, specificity: Specificity) -> std::fmt::Result {
    write!(
        out,
        "({},{},{})",
        specificity.a(),
        specificity.b(),
        specificity.c()
    )
}

fn combinator_label(combinator: Combinator) -> &'static str {
    match combinator {
        Combinator::Descendant => "descendant",
        Combinator::Child => "child",
        Combinator::NextSibling => "next-sibling",
        Combinator::SubsequentSibling => "subsequent-sibling",
    }
}

fn attribute_matcher_label(matcher: AttributeMatcher) -> &'static str {
    match matcher {
        AttributeMatcher::Exact => "exact",
        AttributeMatcher::Includes => "includes",
        AttributeMatcher::DashMatch => "dash-match",
        AttributeMatcher::Prefix => "prefix",
        AttributeMatcher::Suffix => "suffix",
        AttributeMatcher::Substring => "substring",
    }
}

fn write_span_label(out: &mut impl Write, span: Option<CssSpan>) -> std::fmt::Result {
    match span {
        Some(span) => write!(out, "@{}..{}", span.start, span.end),
        None => write!(out, "@<none>"),
    }
}

fn write_quoted(out: &mut impl Write, value: &str) -> std::fmt::Result {
    out.write_char('"')?;
    for ch in value.chars() {
        match ch {
            '\\' => out.write_str("\\\\")?,
            '"' => out.write_str("\\\"")?,
            '\n' => out.write_str("\\n")?,
            '\r' => out.write_str("\\r")?,
            '\t' => out.write_str("\\t")?,
            _ => out.write_char(ch)?,
        }
    }
    out.write_char('"')
}

fn write_indent(out: &mut impl Write, indent: usize) -> std::fmt::Result {
    for _ in 0..indent {
        out.write_char(' ')?;
    }
    Ok(())
}

#[cfg(test)]
mod bounded_tests {
    use super::*;

    fn parsed() -> SelectorListParseResult {
        crate::selectors::parse_selector_source("main > article.featured")
    }

    #[test]
    fn bounded_serializer_matches_canonical_bytes_at_exact_limit() {
        let result = parsed();
        let canonical = serialize_selector_parse_result_for_snapshot(&result);
        assert_eq!(
            serialize_selector_parse_result_for_snapshot_bounded(&result, canonical.len()),
            Ok(canonical.clone())
        );
        assert!(matches!(
            serialize_selector_parse_result_for_snapshot_bounded(&result, canonical.len() - 1),
            Err(SelectorSnapshotSerializationError::LimitExceeded { maximum, observed })
                if maximum == canonical.len() - 1 && observed == canonical.len()
        ));
    }

    #[test]
    fn reservation_failure_returns_no_partial_snapshot() {
        let mut out = BoundedSnapshotWriter::new(usize::MAX);
        out.force_reservation_failure = true;
        let _ = write_snapshot_header(&mut out, "selector-parse");
        assert!(matches!(
            out.finish(),
            Err(SelectorSnapshotSerializationError::ReservationFailure { .. })
        ));
    }
}
