use crate::attributes::AttributeNamespace;
use crate::html5::shared::{AtomTable, Attribute, EngineInvariantError, TextValue, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::{resolve_atom, resolve_text_value};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};
use crate::names::ElementNamespace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum ForeignDispatchDecision {
    Html,
    Foreign,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn foreign_dispatch_decision(
        &self,
        token: &Token,
        atoms: &AtomTable,
    ) -> Result<ForeignDispatchDecision, TreeBuilderError> {
        let Some(adjusted) = self.adjusted_current_node() else {
            return Ok(ForeignDispatchDecision::Html);
        };
        if adjusted.expanded_name.namespace() == ElementNamespace::Html
            || matches!(token, Token::Eof)
        {
            return Ok(ForeignDispatchDecision::Html);
        }
        let local = adjusted.expanded_name.local_name_str();
        let is_start = matches!(token, Token::StartTag { .. });
        let is_text = matches!(token, Token::Text { .. });
        if adjusted.expanded_name.namespace() == ElementNamespace::MathMl
            && matches!(local, "mi" | "mo" | "mn" | "ms" | "mtext")
            && (is_text
                || matches!(token, Token::StartTag { name, .. }
                    if resolve_atom(atoms, *name).is_ok_and(|name| name != "mglyph" && name != "malignmark")))
        {
            return Ok(ForeignDispatchDecision::Html);
        }
        if adjusted.expanded_name.namespace() == ElementNamespace::MathMl
            && local == "annotation-xml"
            && matches!(token, Token::StartTag { name, .. }
                if resolve_atom(atoms, *name).is_ok_and(|name| name == "svg"))
        {
            return Ok(ForeignDispatchDecision::Html);
        }
        if self.is_html_integration_point(adjusted) && (is_start || is_text) {
            return Ok(ForeignDispatchDecision::Html);
        }
        Ok(ForeignDispatchDecision::Foreign)
    }

    fn is_html_integration_point(&self, node: super::AdjustedCurrentNode<'_>) -> bool {
        match node.expanded_name.namespace() {
            ElementNamespace::Svg => matches!(
                node.expanded_name.local_name_str(),
                "foreignObject" | "desc" | "title"
            ),
            ElementNamespace::MathMl if node.expanded_name.local_name_str() == "annotation-xml" => {
                node.attributes.iter().any(|attribute| {
                    attribute.namespace() == AttributeNamespace::None
                        && attribute.local_name() == "encoding"
                        && (attribute.value().eq_ignore_ascii_case("text/html")
                            || attribute
                                .value()
                                .eq_ignore_ascii_case("application/xhtml+xml"))
                })
            }
            _ => false,
        }
    }

    fn current_is_breakout_boundary(&self) -> bool {
        let Some(current) = self.adjusted_current_node() else {
            return true;
        };
        current.expanded_name.namespace() == ElementNamespace::Html
            || (current.expanded_name.namespace() == ElementNamespace::MathMl
                && matches!(
                    current.expanded_name.local_name_str(),
                    "mi" | "mo" | "mn" | "ms" | "mtext"
                ))
            || self.is_html_integration_point(current)
    }

    fn token_has_html_font_breakout_attribute(
        attrs: &[Attribute],
        atoms: &AtomTable,
    ) -> Result<bool, TreeBuilderError> {
        for attribute in attrs {
            if matches!(
                resolve_atom(atoms, attribute.name)?,
                "color" | "face" | "size"
            ) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn breakout_to_html(
        &mut self,
        mode: InsertionMode,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        self.record_parse_error("unexpected-html-token-in-foreign-content", None, Some(mode));
        while !self.current_is_breakout_boundary() {
            self.open_elements.pop().ok_or(EngineInvariantError)?;
        }
        self.dispatch_token_in_html_mode(mode, token, atoms, text)
    }

    pub(in crate::html5::tree_builder) fn process_foreign_token(
        &mut self,
        mode: InsertionMode,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Text { text: token_text } => {
                let resolved = resolve_text_value(token_text, text)?;
                let mut normalized = resolved.to_string();
                let had_null =
                    matches!(token_text, TextValue::NullNormalized { had_null: true, .. })
                        || normalized.contains('\0');
                if had_null {
                    self.record_parse_error(
                        "unexpected-null-character-in-foreign-content",
                        None,
                        Some(mode),
                    );
                    if normalized.contains('\0') {
                        normalized = normalized.replace('\0', "\u{FFFD}");
                    }
                }
                let has_non_whitespace_non_null = match token_text {
                    TextValue::NullNormalized {
                        had_non_whitespace_non_null,
                        ..
                    } => *had_non_whitespace_non_null,
                    _ => normalized.chars().any(|character| {
                        character != '\0' && !matches!(character, '\t' | '\n' | '\x0C' | '\r' | ' ')
                    }),
                };
                if has_non_whitespace_non_null {
                    self.document_state.frameset_ok = false;
                }
                self.insert_text(&TextValue::Owned(normalized), text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_parse_error("doctype-in-foreign-content", None, Some(mode));
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let token_name = resolve_atom(atoms, *name)?;
                if super::is_foreign_breakout_start(token_name)
                    || (token_name == "font"
                        && Self::token_has_html_font_breakout_attribute(attrs, atoms)?)
                {
                    return self.breakout_to_html(mode, token, atoms, text);
                }
                let namespace = self
                    .adjusted_current_node_namespace()
                    .ok_or(EngineInvariantError)?;
                let local = if namespace == ElementNamespace::Svg {
                    super::svg_adjusted_tag_name(token_name)
                } else {
                    token_name
                };
                let adjusted_name = atoms.lookup_exact(local).ok_or(EngineInvariantError)?;
                let adjusted = super::adjust_foreign_attributes(namespace, attrs, atoms, text)?;
                self.internal_post_adjustment_attribute_collisions = self
                    .internal_post_adjustment_attribute_collisions
                    .saturating_add(adjusted.post_adjustment_collisions as u64);
                let _ = self.insert_foreign_element(
                    namespace,
                    adjusted_name,
                    adjusted.attributes,
                    *self_closing,
                    atoms,
                )?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } => {
                let token_name = resolve_atom(atoms, *name)?;
                if matches!(token_name, "br" | "p") {
                    return self.breakout_to_html(mode, token, atoms, text);
                }
                let current = self.open_elements.current().ok_or(EngineInvariantError)?;
                let current_name = resolve_atom(atoms, current.name())?;
                if !current_name.eq_ignore_ascii_case(token_name) {
                    self.record_parse_error(
                        "foreign-end-tag-current-node-mismatch",
                        Some(*name),
                        Some(mode),
                    );
                }
                for index in (0..self.open_elements.len()).rev() {
                    let candidate = self.open_elements.get(index).ok_or(EngineInvariantError)?;
                    if candidate.namespace() == ElementNamespace::Html {
                        return self.dispatch_token_in_html_mode(mode, token, atoms, text);
                    }
                    if resolve_atom(atoms, candidate.name())?.eq_ignore_ascii_case(token_name) {
                        while self.open_elements.len() > index {
                            self.open_elements.pop().ok_or(EngineInvariantError)?;
                        }
                        return Ok(DispatchOutcome::Done);
                    }
                }
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => self.dispatch_token_in_html_mode(mode, token, atoms, text),
        }
    }
}
