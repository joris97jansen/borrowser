use crate::tokenizer::TokenizerView;
use crate::{Token, TokenStream};
use std::fmt::Write;

pub(crate) fn token_snapshot(stream: &TokenStream) -> Vec<String> {
    let atoms = stream.atoms();
    stream
        .tokens()
        .iter()
        .map(|token| match token {
            Token::Doctype(value) => format!("Doctype({})", stream.payload_text(value)),
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let mut line = String::new();
                let _ = write!(&mut line, "StartTag({}", atoms.resolve(*name));
                for (attr, value) in attributes {
                    line.push(' ');
                    line.push_str(atoms.resolve(*attr));
                    if let Some(value) = value {
                        line.push_str("=\"");
                        line.push_str(stream.attr_value(value));
                        line.push('"');
                    }
                }
                if *self_closing {
                    line.push_str(" /");
                }
                line.push(')');
                line
            }
            Token::EndTag(name) => format!("EndTag({})", atoms.resolve(*name)),
            Token::Comment(text) => format!("Comment({})", stream.payload_text(text)),
            Token::TextSpan { .. } | Token::TextOwned { .. } => {
                let text = stream.text(token).unwrap_or("");
                format!("Text({text})")
            }
        })
        .collect()
}

pub(crate) fn token_snapshot_with_view(view: TokenizerView<'_>, tokens: &[Token]) -> Vec<String> {
    tokens
        .iter()
        .map(|token| match token {
            Token::Doctype(value) => format!("Doctype({})", view.payload_text(value)),
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let mut line = String::new();
                let _ = write!(&mut line, "StartTag({}", view.resolve_atom(*name));
                for (attr, value) in attributes {
                    line.push(' ');
                    line.push_str(view.resolve_atom(*attr));
                    if let Some(value) = value {
                        line.push_str("=\"");
                        line.push_str(view.attr_value(value));
                        line.push('"');
                    }
                }
                if *self_closing {
                    line.push_str(" /");
                }
                line.push(')');
                line
            }
            Token::EndTag(name) => format!("EndTag({})", view.resolve_atom(*name)),
            Token::Comment(text) => format!("Comment({})", view.payload_text(text)),
            Token::TextSpan { .. } | Token::TextOwned { .. } => {
                let text = view.text(token).unwrap_or("");
                format!("Text({text})")
            }
        })
        .collect()
}
