use crate::{Node, build_dom, tokenize};
use tools::utf8::{finish_utf8, push_utf8_chunk};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryPolicy {
    EnforceUtf8,
    AllowUnaligned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChunkPlan {
    Fixed(usize),
    Sizes(Vec<usize>),
    Boundaries {
        indices: Vec<usize>,
        policy: BoundaryPolicy,
    },
}

impl ChunkPlan {
    pub fn fixed(size: usize) -> Self {
        Self::Fixed(size)
    }

    pub fn sizes(sizes: impl Into<Vec<usize>>) -> Self {
        Self::Sizes(sizes.into())
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
            ChunkPlan::Fixed(size) => {
                assert!(*size > 0, "chunk size must be > 0");
                let mut offset = 0usize;
                while offset < bytes.len() {
                    let end = (offset + size).min(bytes.len());
                    f(&bytes[offset..end]);
                    offset = end;
                }
            }
            ChunkPlan::Sizes(sizes) => {
                let mut offset = 0usize;
                for size in sizes {
                    assert!(*size > 0, "chunk size must be > 0");
                    if offset >= bytes.len() {
                        break;
                    }
                    let end = (offset + size).min(bytes.len());
                    f(&bytes[offset..end]);
                    offset = end;
                }
                if offset < bytes.len() {
                    f(&bytes[offset..]);
                }
            }
            ChunkPlan::Boundaries { indices, policy } => {
                let mut points = normalize_boundaries(input, indices, *policy);
                points.sort_unstable();
                points.dedup();
                points.retain(|&idx| idx > 0 && idx < bytes.len());
                let mut last = 0usize;
                for idx in points {
                    if idx > last {
                        f(&bytes[last..idx]);
                    }
                    last = idx;
                }
                if last < bytes.len() {
                    f(&bytes[last..]);
                }
            }
        }
    }
}

fn normalize_boundaries(input: &str, indices: &[usize], policy: BoundaryPolicy) -> Vec<usize> {
    if matches!(policy, BoundaryPolicy::AllowUnaligned) {
        return indices.to_vec();
    }
    indices
        .iter()
        .copied()
        .filter(|&idx| input.is_char_boundary(idx))
        .collect()
}

pub fn run_full(input: &str) -> Node {
    let stream = tokenize(input);
    build_dom(&stream)
}

pub fn run_chunked(input: &str, plan: &ChunkPlan) -> Node {
    let mut text = String::new();
    let mut carry = Vec::new();
    plan.for_each_chunk(input, |chunk| {
        push_utf8_chunk(&mut text, &mut carry, chunk);
    });
    finish_utf8(&mut text, &mut carry);
    let stream = tokenize(&text);
    build_dom(&stream)
}

#[cfg(test)]
mod tests {
    use super::{ChunkPlan, run_chunked, run_full};
    use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};

    #[test]
    fn chunked_fixed_matches_full() {
        let input = "<p>café &amp; crème</p>";
        let full = run_full(input);
        let chunked = run_chunked(input, &ChunkPlan::fixed(1));
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
}
