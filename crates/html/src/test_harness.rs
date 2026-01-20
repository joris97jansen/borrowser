use crate::tokenizer::Tokenizer;
use crate::{Node, build_dom, tokenize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryPolicy {
    EnforceUtf8,
    AllowUnaligned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChunkPlan {
    Fixed {
        size: usize,
        policy: BoundaryPolicy,
    },
    Sizes {
        sizes: Vec<usize>,
        policy: BoundaryPolicy,
    },
    Boundaries {
        indices: Vec<usize>,
        policy: BoundaryPolicy,
    },
}

impl ChunkPlan {
    pub fn fixed(size: usize) -> Self {
        Self::Fixed {
            size,
            policy: BoundaryPolicy::EnforceUtf8,
        }
    }

    pub fn fixed_unaligned(size: usize) -> Self {
        Self::Fixed {
            size,
            policy: BoundaryPolicy::AllowUnaligned,
        }
    }

    pub fn sizes(sizes: impl Into<Vec<usize>>) -> Self {
        Self::Sizes {
            sizes: sizes.into(),
            policy: BoundaryPolicy::EnforceUtf8,
        }
    }

    pub fn sizes_unaligned(sizes: impl Into<Vec<usize>>) -> Self {
        Self::Sizes {
            sizes: sizes.into(),
            policy: BoundaryPolicy::AllowUnaligned,
        }
    }

    pub fn boundaries(indices: impl Into<Vec<usize>>) -> Self {
        Self::Boundaries {
            indices: indices.into(),
            policy: BoundaryPolicy::EnforceUtf8,
        }
    }

    pub fn boundaries_unaligned(indices: impl Into<Vec<usize>>) -> Self {
        Self::Boundaries {
            indices: indices.into(),
            policy: BoundaryPolicy::AllowUnaligned,
        }
    }

    fn for_each_chunk(&self, input: &str, mut f: impl FnMut(&[u8])) {
        let bytes = input.as_bytes();
        match self {
            ChunkPlan::Fixed { size, policy } => {
                assert!(*size > 0, "chunk size must be > 0");
                let mut offset = 0usize;
                while offset < bytes.len() {
                    let end = (offset + size).min(bytes.len());
                    assert_chunk_boundary(input, offset, *policy, "fixed-start");
                    assert_chunk_boundary(input, end, *policy, "fixed-end");
                    f(&bytes[offset..end]);
                    offset = end;
                }
            }
            ChunkPlan::Sizes { sizes, policy } => {
                let mut offset = 0usize;
                for size in sizes {
                    assert!(*size > 0, "chunk size must be > 0");
                    if offset >= bytes.len() {
                        break;
                    }
                    let end = (offset + size).min(bytes.len());
                    assert_chunk_boundary(input, offset, *policy, "sizes-start");
                    assert_chunk_boundary(input, end, *policy, "sizes-end");
                    f(&bytes[offset..end]);
                    offset = end;
                }
                if offset < bytes.len() {
                    assert_chunk_boundary(input, offset, *policy, "sizes-final-start");
                    assert_chunk_boundary(input, bytes.len(), *policy, "sizes-final-end");
                    f(&bytes[offset..]);
                }
            }
            ChunkPlan::Boundaries { indices, policy } => {
                let validated = matches!(policy, BoundaryPolicy::EnforceUtf8);
                // Boundaries are normalized (sorted, deduped, clipped to (0, len)).
                let mut points = validate_boundaries_utf8(input, indices, *policy);
                points.sort_unstable();
                points.dedup();
                points.retain(|&idx| idx > 0 && idx < bytes.len());
                let mut last = 0usize;
                for idx in points {
                    if !validated {
                        assert_chunk_boundary(input, last, *policy, "boundaries-start");
                        assert_chunk_boundary(input, idx, *policy, "boundaries-end");
                    }
                    if idx > last {
                        f(&bytes[last..idx]);
                    }
                    last = idx;
                }
                if last < bytes.len() {
                    if !validated {
                        assert_chunk_boundary(input, last, *policy, "boundaries-final-start");
                        assert_chunk_boundary(input, bytes.len(), *policy, "boundaries-final-end");
                    }
                    f(&bytes[last..]);
                }
            }
        }
    }
}

fn assert_chunk_boundary(input: &str, idx: usize, policy: BoundaryPolicy, context: &str) {
    if matches!(policy, BoundaryPolicy::EnforceUtf8) {
        assert!(
            input.is_char_boundary(idx),
            "chunk boundary must be UTF-8 aligned ({context}): {idx}"
        );
    }
}

fn validate_boundaries_utf8(input: &str, indices: &[usize], policy: BoundaryPolicy) -> Vec<usize> {
    if matches!(policy, BoundaryPolicy::AllowUnaligned) {
        return indices.to_vec();
    }
    for &idx in indices {
        assert!(
            input.is_char_boundary(idx),
            "boundary must be UTF-8 aligned: {idx}"
        );
    }
    indices.to_vec()
}

pub fn run_full(input: &str) -> Node {
    let stream = tokenize(input);
    build_dom(&stream)
}

pub fn run_chunked(input: &str, plan: &ChunkPlan) -> Node {
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::new();
    plan.for_each_chunk(input, |chunk| {
        tokenizer.feed(chunk);
        tokenizer.drain_into(&mut tokens);
    });
    tokenizer.finish();
    tokenizer.drain_into(&mut tokens);
    let (atoms, source, text_pool) = tokenizer.into_parts();
    let stream = crate::TokenStream::new(tokens, atoms, source, text_pool);
    build_dom(&stream)
}

#[cfg(test)]
mod tests {
    use super::{ChunkPlan, run_chunked, run_full};
    use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
    use crate::tokenizer::Tokenizer;
    use std::fmt::Write;

    fn token_snapshot(stream: &crate::TokenStream) -> Vec<String> {
        let atoms = stream.atoms();
        stream
            .tokens()
            .iter()
            .map(|token| match token {
                crate::Token::Doctype(value) => format!("Doctype({value})"),
                crate::Token::StartTag {
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
                            line.push_str(value);
                            line.push('"');
                        }
                    }
                    if *self_closing {
                        line.push_str(" /");
                    }
                    line.push(')');
                    line
                }
                crate::Token::EndTag(name) => format!("EndTag({})", atoms.resolve(*name)),
                crate::Token::Comment(text) => format!("Comment({text})"),
                crate::Token::TextSpan { .. } | crate::Token::TextOwned { .. } => {
                    let text = stream.text(token).unwrap_or("");
                    format!("Text({text})")
                }
            })
            .collect()
    }

    #[test]
    fn chunked_fixed_matches_full() {
        let input = "<p>café &amp; crème</p>";
        let full = run_full(input);
        let chunked = run_chunked(input, &ChunkPlan::fixed_unaligned(1));
        assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
    }

    #[test]
    fn chunked_boundary_plan_allows_unaligned_splits() {
        let input = "<p>é</p>";
        let boundaries = vec![1, 2];
        let full = run_full(input);
        let chunked = run_chunked(input, &ChunkPlan::boundaries_unaligned(boundaries));
        assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
    }

    #[test]
    fn chunked_boundary_splits_utf8_codepoint() {
        let input = "<p>é</p>";
        let boundaries = vec![4];
        let full = run_full(input);
        let chunked = run_chunked(input, &ChunkPlan::boundaries_unaligned(boundaries));
        assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
    }

    #[test]
    fn chunked_boundary_splits_comment_terminator() {
        let input = "<!--x-->";
        let boundaries = vec!["<!--x--".len()];
        let full = run_full(input);
        let chunked = run_chunked(input, &ChunkPlan::boundaries(boundaries));
        assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
    }

    #[test]
    fn chunked_boundary_splits_rawtext_close_tag() {
        let input = "<script>hi</script>";
        let boundaries = vec!["<script>hi</scr".len()];
        let full = run_full(input);
        let chunked = run_chunked(input, &ChunkPlan::boundaries(boundaries));
        assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
    }

    #[test]
    fn chunked_draining_leaves_no_tokens_behind() {
        let input = "<div>ok</div><!--x-->";
        let bytes = input.as_bytes();
        let sizes = [2, 3, 1];
        let mut tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        let mut offset = 0usize;

        for size in sizes {
            if offset >= bytes.len() {
                break;
            }
            let end = (offset + size).min(bytes.len());
            tokenizer.feed(&bytes[offset..end]);
            tokenizer.drain_into(&mut tokens);
            offset = end;
        }
        if offset < bytes.len() {
            tokenizer.feed(&bytes[offset..]);
        }
        tokenizer.finish();
        tokenizer.drain_into(&mut tokens);

        assert!(
            tokenizer.drain_tokens().is_empty(),
            "expected tokenizer to have no buffered tokens after draining"
        );

        let (atoms, source, text_pool) = tokenizer.into_parts();
        let stream = crate::TokenStream::new(tokens, atoms, source, text_pool);
        let expected = crate::tokenize(input);
        assert_eq!(
            token_snapshot(&expected),
            token_snapshot(&stream),
            "expected drained tokens to match full tokenize() snapshot"
        );
    }
}
