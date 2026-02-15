use html::html5::{AttributeValue, DocumentParseContext, TextResolver, Token};

use crate::support::escape_text;

pub struct TokenFormatContext<'a> {
    pub case_id: &'a str,
    pub mode: &'a str,
}

pub fn format_tokens(
    tokens: &[Token],
    resolver: &impl TextResolver,
    ctx: &DocumentParseContext,
    context: &TokenFormatContext<'_>,
    index: &mut usize,
    mut saw_eof_token: Option<&mut bool>,
) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let token_index = *index;
        *index = index.saturating_add(1);
        if matches!(token, Token::Eof)
            && let Some(saw_eof) = saw_eof_token.as_deref_mut()
        {
            *saw_eof = true;
        }
        let line = match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                let name = match name {
                    None => "null".to_string(),
                    Some(id) => ctx
                        .atoms
                        .resolve(*id)
                        .ok_or_else(|| {
                            format!(
                                "unknown atom id in doctype name for '{}' [{}] token #{}: {id:?}",
                                context.case_id, context.mode, token_index
                            )
                        })?
                        .to_string(),
                };
                let public_id = public_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                let system_id = system_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                format!(
                    "DOCTYPE name={name} public_id={public_id} system_id={system_id} force_quirks={force_quirks}"
                )
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let name = ctx.atoms.resolve(*name).ok_or_else(|| {
                    format!(
                        "unknown atom id in start tag for '{}' [{}] token #{}: {name:?}",
                        context.case_id, context.mode, token_index
                    )
                })?;
                let mut line = String::new();
                line.push_str("START name=");
                line.push_str(name);
                line.push_str(" attrs=[");
                for (attr_index, attr) in attributes.iter().enumerate() {
                    if attr_index > 0 {
                        line.push(' ');
                    }
                    line.push_str(
                        format_attr(attr, resolver, ctx, context, token_index, attr_index)?
                            .as_str(),
                    );
                }
                line.push_str("] self_closing=");
                line.push_str(if *self_closing { "true" } else { "false" });
                line
            }
            Token::EndTag { name } => {
                let name = ctx.atoms.resolve(*name).ok_or_else(|| {
                    format!(
                        "unknown atom id in end tag for '{}' [{}] token #{}: {name:?}",
                        context.case_id, context.mode, token_index
                    )
                })?;
                format!("END name={name}")
            }
            Token::Comment { text } => {
                let text = resolver.resolve_span(*text);
                format!("COMMENT text=\"{}\"", escape_text(text))
            }
            Token::Character { span } => {
                let text = resolver.resolve_span(*span);
                format!("CHAR text=\"{}\"", escape_text(text))
            }
            Token::Eof => "EOF".to_string(),
        };
        out.push(line);
    }
    Ok(out)
}

fn format_attr(
    attr: &html::html5::Attribute,
    resolver: &impl TextResolver,
    ctx: &DocumentParseContext,
    context: &TokenFormatContext<'_>,
    token_index: usize,
    attr_index: usize,
) -> Result<String, String> {
    let name = ctx.atoms.resolve(attr.name).ok_or_else(|| {
        format!(
            "unknown atom id in attribute for '{}' [{}] token #{} attr #{}: {:?}",
            context.case_id, context.mode, token_index, attr_index, attr.name
        )
    })?;
    match &attr.value {
        None => Ok(name.to_string()),
        Some(AttributeValue::Span(span)) => {
            let value = resolver.resolve_span(*span);
            Ok(format!("{name}=\"{}\"", escape_text(value)))
        }
        Some(AttributeValue::Owned(value)) => Ok(format!("{name}=\"{}\"", escape_text(value))),
    }
}
