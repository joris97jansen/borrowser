use super::super::Tokenizer;
use super::super::capacity::estimate_token_capacity;
use super::super::scan::clamp_char_boundary;
use crate::types::{Token, TokenStream};

pub(super) fn text_eq(stream: &TokenStream, token: &Token, expected: &str) -> bool {
    stream.text(token) == Some(expected)
}

pub(super) fn tokenize_in_chunks(input: &str, sizes: &[usize]) -> TokenStream {
    let bytes = input.as_bytes();
    let mut tokenizer = Tokenizer::new();
    let mut offset = 0usize;
    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        offset = end;
    }
    if offset < bytes.len() {
        tokenizer.feed(&bytes[offset..]);
    }
    tokenizer.finish();
    tokenizer.into_stream()
}

pub(super) fn tokenize_with_push_str(input: &str, sizes: &[usize]) -> TokenStream {
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::with_capacity(estimate_token_capacity(input.len()));
    let mut offset = 0usize;
    for size in sizes {
        if offset >= input.len() {
            break;
        }
        let end = (offset + size).min(input.len());
        let end = clamp_char_boundary(input, end, offset);
        if end == offset {
            break;
        }
        tokenizer.push_str_into(&input[offset..end], &mut tokens);
        offset = end;
    }
    if offset < input.len() {
        tokenizer.push_str_into(&input[offset..], &mut tokens);
    }
    tokenizer.finish_into(&mut tokens);
    let (atoms, source, text_pool) = tokenizer.into_parts();
    TokenStream::new(tokens, atoms, source, text_pool)
}

pub(super) fn tokenize_with_feed_bytes(bytes: &[u8], split: usize) -> TokenStream {
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::with_capacity(estimate_token_capacity(bytes.len()));
    tokenizer.feed(&bytes[..split]);
    tokenizer.drain_into(&mut tokens);
    tokenizer.feed(&bytes[split..]);
    tokenizer.finish();
    tokenizer.drain_into(&mut tokens);
    let (atoms, source, text_pool) = tokenizer.into_parts();
    TokenStream::new(tokens, atoms, source, text_pool)
}
