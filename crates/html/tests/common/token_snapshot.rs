use html::html5::{DocumentParseContext, TextResolver, Token, TokenFmt};

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
    let fmt = TokenFmt::new(&ctx.atoms, resolver);
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let token_index = *index;
        *index = index.saturating_add(1);
        if matches!(token, Token::Eof)
            && let Some(saw_eof) = saw_eof_token.as_deref_mut()
        {
            *saw_eof = true;
        }
        let line = fmt.format_token(token).map_err(|err| {
            format!(
                "{err} for '{}' [{}] token #{}",
                context.case_id, context.mode, token_index
            )
        })?;
        out.push(line);
    }
    Ok(out)
}
