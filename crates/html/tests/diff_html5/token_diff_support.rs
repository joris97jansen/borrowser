use super::{CaseContext, ensure_need_more_input_only_at_buffer_end, validate_tokenize_result};
use html::TokenStream;
use html::html5::{
    AttributeValue, DocumentParseContext, Html5Tokenizer, Input, TextResolver, TextValue, Token,
    TokenizeResult, TokenizerConfig,
};
use html_test_support::escape_text;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NormToken {
    Doctype {
        name: Option<String>,
    },
    StartTag {
        name: String,
        attrs: Vec<(String, Option<String>)>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Comment {
        text: String,
    },
    Char {
        text: String,
    },
    Eof,
}

pub(super) struct Html5TokenDiffDriver<'a> {
    case: CaseContext<'a>,
    strict: bool,
}

impl<'a> Html5TokenDiffDriver<'a> {
    pub(super) fn new(case: CaseContext<'a>, strict: bool) -> Self {
        Self { case, strict }
    }

    pub(super) fn collect_normalized_html5_tokens(
        &self,
        input_html: &str,
    ) -> Result<Vec<NormToken>, String> {
        let mut ctx = DocumentParseContext::new();
        let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
        let mut input = Input::new();
        let mut saw_eof_token = false;
        let mut out = Vec::new();

        input.push_str(input_html);
        loop {
            let consumed_before = tokenizer.stats().bytes_consumed;
            let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let consumed_after = tokenizer.stats().bytes_consumed;
            validate_tokenize_result(result, "push_input").map_err(|err| {
                format!(
                    "tokenizer error in '{}' at {:?}: {err}",
                    self.case.id, self.case.path
                )
            })?;
            ensure_need_more_input_only_at_buffer_end(
                self.case,
                result,
                consumed_before,
                consumed_after,
                input.as_str().len(),
            )?;
            self.collect_normalized_batch(
                &mut out,
                &mut tokenizer,
                &mut input,
                &ctx,
                &mut saw_eof_token,
            )?;
            if matches!(result, TokenizeResult::NeedMoreInput) {
                break;
            }
        }

        validate_tokenize_result(tokenizer.finish(&input), "finish").map_err(|err| {
            format!(
                "tokenizer error in '{}' at {:?}: {err}",
                self.case.id, self.case.path
            )
        })?;
        self.collect_normalized_batch(
            &mut out,
            &mut tokenizer,
            &mut input,
            &ctx,
            &mut saw_eof_token,
        )?;
        if !saw_eof_token {
            return Err(format!(
                "expected EOF token but none was observed (case '{}' at {:?})",
                self.case.id, self.case.path
            ));
        }
        Ok(out)
    }

    fn collect_normalized_batch(
        &self,
        out: &mut Vec<NormToken>,
        tokenizer: &mut Html5Tokenizer,
        input: &mut Input,
        ctx: &DocumentParseContext,
        saw_eof_token: &mut bool,
    ) -> Result<(), String> {
        loop {
            let batch = tokenizer.next_batch(input);
            let resolver = batch.resolver();
            let mut saw_any = false;
            for token in batch.iter() {
                saw_any = true;
                match token {
                    Token::Doctype {
                        name,
                        public_id: _,
                        system_id: _,
                        force_quirks: _,
                    } => {
                        let name = match name {
                            None => None,
                            Some(id) => {
                                let resolved = ctx.atoms.resolve(*id).unwrap_or("");
                                if resolved.is_empty() && self.strict {
                                    return Err(format!(
                                        "empty doctype name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={id:?})",
                                        self.case.id, self.case.path
                                    ));
                                }
                                if resolved.is_empty() {
                                    None
                                } else {
                                    Some(resolved.to_ascii_lowercase())
                                }
                            }
                        };
                        out.push(NormToken::Doctype { name });
                    }
                    Token::StartTag {
                        name,
                        attrs,
                        self_closing,
                    } => {
                        let raw_name_id = *name;
                        let name = ctx.atoms.resolve(*name).unwrap_or("");
                        if name.is_empty() && self.strict {
                            return Err(format!(
                                "empty start tag name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={raw_name_id:?})",
                                self.case.id, self.case.path
                            ));
                        }
                        let name = name.to_ascii_lowercase();
                        let mut attrs_out: Vec<(String, Option<String>, usize)> =
                            Vec::with_capacity(attrs.len());
                        for (index, attr) in attrs.iter().enumerate() {
                            let attr_name = ctx.atoms.resolve(attr.name).unwrap_or("");
                            if attr_name.is_empty() && self.strict {
                                return Err(format!(
                                    "empty attribute name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={:?})",
                                    self.case.id, self.case.path, attr.name
                                ));
                            }
                            let attr_name = attr_name.to_ascii_lowercase();
                            let value = match &attr.value {
                                None => None,
                                Some(AttributeValue::Span(span)) => Some(
                                    resolver
                                        .resolve_span(*span)
                                        .map_err(|err| {
                                            format!(
                                                "invalid attribute value span in '{}' (attr {}) at {:?}: {err:?}",
                                                self.case.id, attr_name, self.case.path
                                            )
                                        })?
                                        .to_string(),
                                ),
                                Some(AttributeValue::Owned(value)) => Some(value.clone()),
                            };
                            attrs_out.push((attr_name, value, index));
                        }
                        attrs_out.sort_by(
                            |(a_name, a_value, a_index), (b_name, b_value, b_index)| {
                                let cmp = a_name
                                    .cmp(b_name)
                                    .then_with(|| a_value.as_deref().cmp(&b_value.as_deref()));
                                if cmp == std::cmp::Ordering::Equal {
                                    a_index.cmp(b_index)
                                } else {
                                    cmp
                                }
                            },
                        );
                        let attrs = attrs_out
                            .into_iter()
                            .map(|(name, value, _)| (name, value))
                            .collect();
                        // Diff normalization only: treat HTML void elements as self-closing
                        // to reduce cross-implementation noise in token comparisons.
                        let self_closing = *self_closing || is_html_void_tag(&name);
                        out.push(NormToken::StartTag {
                            name,
                            attrs,
                            self_closing,
                        });
                    }
                    Token::EndTag { name } => {
                        let raw_name_id = *name;
                        let name = ctx.atoms.resolve(*name).unwrap_or("");
                        if name.is_empty() && self.strict {
                            return Err(format!(
                                "empty end tag name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={raw_name_id:?})",
                                self.case.id, self.case.path
                            ));
                        }
                        out.push(NormToken::EndTag {
                            name: name.to_ascii_lowercase(),
                        });
                    }
                    Token::Comment { text } => {
                        let text = match text {
                            TextValue::Span(span) => {
                                resolver.resolve_span(*span).map_err(|err| {
                                    format!(
                                        "invalid comment span in '{}' at {:?}: {err:?}",
                                        self.case.id, self.case.path
                                    )
                                })?
                            }
                            TextValue::Owned(text) => text.as_str(),
                        };
                        out.push(NormToken::Comment {
                            text: text.to_string(),
                        });
                    }
                    Token::Text { text } => {
                        let text = match text {
                            TextValue::Span(span) => {
                                resolver.resolve_span(*span).map_err(|err| {
                                    format!(
                                        "invalid text span in '{}' at {:?}: {err:?}",
                                        self.case.id, self.case.path
                                    )
                                })?
                            }
                            TextValue::Owned(value) => value.as_str(),
                        };
                        push_char(out, text);
                    }
                    Token::Eof => {
                        if !*saw_eof_token {
                            *saw_eof_token = true;
                            out.push(NormToken::Eof);
                        }
                    }
                }
            }
            if !saw_any {
                break;
            }
        }
        Ok(())
    }
}

pub(super) fn html5_only_eof(tokens: &[NormToken]) -> bool {
    matches!(tokens, [NormToken::Eof])
}

pub(super) fn normalize_simplified_tokens(stream: &TokenStream) -> Vec<NormToken> {
    let mut out = Vec::with_capacity(stream.tokens().len());
    for token in stream.tokens() {
        match token {
            html::Token::Doctype(payload) => {
                let name = normalize_simplified_doctype_name(stream.payload_text(payload));
                out.push(NormToken::Doctype { name });
            }
            html::Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let name = stream.atoms().resolve(*name).to_ascii_lowercase();
                let mut attrs = Vec::with_capacity(attributes.len());
                for (index, (attr, value)) in attributes.iter().enumerate() {
                    let attr_name = stream.atoms().resolve(*attr).to_ascii_lowercase();
                    let value = value
                        .as_ref()
                        .map(|value| stream.attr_value(value).to_string());
                    attrs.push((attr_name, value, index));
                }
                attrs.sort_by(|(a_name, a_value, a_index), (b_name, b_value, b_index)| {
                    let cmp = a_name
                        .cmp(b_name)
                        .then_with(|| a_value.as_deref().cmp(&b_value.as_deref()));
                    if cmp == std::cmp::Ordering::Equal {
                        a_index.cmp(b_index)
                    } else {
                        cmp
                    }
                });
                let attrs = attrs
                    .into_iter()
                    .map(|(name, value, _)| (name, value))
                    .collect();
                // Diff normalization only: treat HTML void elements as self-closing
                // to reduce cross-implementation noise in token comparisons.
                let self_closing = *self_closing || is_html_void_tag(&name);
                out.push(NormToken::StartTag {
                    name,
                    attrs,
                    self_closing,
                });
            }
            html::Token::EndTag(name) => {
                let name = stream.atoms().resolve(*name).to_ascii_lowercase();
                out.push(NormToken::EndTag { name });
            }
            html::Token::Comment(payload) => {
                let text = stream.payload_text(payload).to_string();
                out.push(NormToken::Comment { text });
            }
            html::Token::TextSpan { .. } | html::Token::TextOwned { .. } => {
                let text = stream.text(token).unwrap_or("");
                push_char(&mut out, text);
            }
        }
    }
    if !matches!(out.last(), Some(NormToken::Eof)) {
        out.push(NormToken::Eof);
    }
    out
}

fn normalize_simplified_doctype_name(raw: &str) -> Option<String> {
    let mut text = raw.trim();
    if text.is_empty() {
        return None;
    }

    // Legacy tokenizer payloads may include the keyword itself (e.g. "DOCTYPE html").
    // Normalize to the semantic doctype name used by the html5 tokenizer diff path.
    if text.len() >= 7
        && text
            .get(..7)
            .is_some_and(|head| head.eq_ignore_ascii_case("doctype"))
    {
        let after_kw = &text[7..];
        if after_kw
            .chars()
            .next()
            .is_none_or(|ch| ch.is_ascii_whitespace())
        {
            text = after_kw.trim_start();
        }
    }

    let name = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.is_empty() { None } else { Some(name) }
}

fn push_char(tokens: &mut Vec<NormToken>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(NormToken::Char { text: existing }) = tokens.last_mut() {
        existing.push_str(text);
        return;
    }
    tokens.push(NormToken::Char {
        text: text.to_string(),
    });
}

pub(super) fn format_norm_tokens(tokens: &[NormToken]) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let line = match token {
            NormToken::Doctype { name } => {
                let name = name.as_deref().unwrap_or("null");
                format!("DOCTYPE name={name}")
            }
            NormToken::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let mut line = String::new();
                line.push_str("START name=");
                line.push_str(name);
                line.push_str(" attrs=[");
                for (index, (attr, value)) in attrs.iter().enumerate() {
                    if index > 0 {
                        line.push(' ');
                    }
                    line.push_str(attr);
                    if let Some(value) = value {
                        line.push_str("=\"");
                        line.push_str(&escape_text(value));
                        line.push('"');
                    }
                }
                line.push_str("] self_closing=");
                line.push_str(if *self_closing { "true" } else { "false" });
                line
            }
            NormToken::EndTag { name } => format!("END name={name}"),
            NormToken::Comment { text } => format!("COMMENT text=\"{}\"", escape_text(text)),
            NormToken::Char { text } => format!("CHAR text=\"{}\"", escape_text(text)),
            NormToken::Eof => "EOF".to_string(),
        };
        out.push(line);
    }
    out
}

fn is_html_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}
