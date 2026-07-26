//! Canonical token observation at the production queue-drain boundary.

use super::{TextResolver, TokenFmt, TokenFmtError};
use crate::html5::shared::{AtomTable, ObservedToken, ObservedTokenAttribute, Token};

pub(super) fn canonicalize_token(
    token: &Token,
    atoms: &AtomTable,
    resolver: &dyn TextResolver,
) -> Result<ObservedToken, TokenFmtError> {
    let fmt = TokenFmt::new(atoms, resolver);
    match token {
        Token::Doctype {
            name,
            public_id,
            system_id,
            force_quirks,
        } => Ok(ObservedToken::Doctype {
            name: name
                .map(|name| fmt.resolve_atom(name).map(str::to_string))
                .transpose()?,
            public_id: public_id.clone(),
            system_id: system_id.clone(),
            force_quirks: *force_quirks,
        }),
        Token::StartTag {
            name,
            attrs,
            self_closing,
        } => Ok(ObservedToken::StartTag {
            name: fmt.resolve_atom(*name)?.to_string(),
            attributes: attrs
                .iter()
                .map(|attribute| {
                    Ok(ObservedTokenAttribute {
                        name: fmt.resolve_atom(attribute.name)?.to_string(),
                        value: fmt.resolve_attr_value(&attribute.value)?.into_owned(),
                    })
                })
                .collect::<Result<Vec<_>, TokenFmtError>>()?,
            self_closing: *self_closing,
        }),
        Token::EndTag { name } => Ok(ObservedToken::EndTag {
            name: fmt.resolve_atom(*name)?.to_string(),
        }),
        Token::Comment { text } => Ok(ObservedToken::Comment {
            data: fmt.resolve_text_value(text)?.into_owned(),
        }),
        Token::ProcessingInstruction(processing_instruction) => {
            Ok(ObservedToken::ProcessingInstruction {
                target: processing_instruction.target.clone(),
                data: fmt
                    .resolve_text_value(&processing_instruction.data)?
                    .into_owned(),
            })
        }
        Token::Text { text } => Ok(ObservedToken::Character {
            data: fmt.resolve_text_value(text)?.into_owned(),
        }),
        Token::Eof => Ok(ObservedToken::Eof),
    }
}
