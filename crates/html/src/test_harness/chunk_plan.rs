use super::fuzz::{every_byte_boundaries, semantic_boundaries};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryPolicy {
    /// Enforce UTF-8 aligned boundaries between chunks.
    Utf8Aligned,
    /// Byte-stream mode; tokenizer must handle partial UTF-8 sequences.
    ByteStream,
}

impl fmt::Display for BoundaryPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoundaryPolicy::Utf8Aligned => f.write_str("utf8"),
            BoundaryPolicy::ByteStream => f.write_str("bytes"),
        }
    }
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

impl fmt::Display for ChunkPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunkPlan::Fixed { size, policy } => {
                write!(f, "fixed size={size} policy={policy}")
            }
            ChunkPlan::Sizes { sizes, policy } => {
                write!(
                    f,
                    "sizes count={} policy={policy} sizes={sizes:?}",
                    sizes.len()
                )
            }
            ChunkPlan::Boundaries { indices, policy } => {
                write!(
                    f,
                    "boundaries count={} policy={policy} indices={indices:?}",
                    indices.len()
                )
            }
        }
    }
}

impl ChunkPlan {
    pub fn fixed(size: usize) -> Self {
        Self::Fixed {
            size,
            policy: BoundaryPolicy::Utf8Aligned,
        }
    }

    pub fn fixed_unaligned(size: usize) -> Self {
        Self::Fixed {
            size,
            policy: BoundaryPolicy::ByteStream,
        }
    }

    pub fn sizes(sizes: impl Into<Vec<usize>>) -> Self {
        Self::Sizes {
            sizes: sizes.into(),
            policy: BoundaryPolicy::Utf8Aligned,
        }
    }

    pub fn sizes_unaligned(sizes: impl Into<Vec<usize>>) -> Self {
        Self::Sizes {
            sizes: sizes.into(),
            policy: BoundaryPolicy::ByteStream,
        }
    }

    pub fn boundaries(indices: impl Into<Vec<usize>>) -> Self {
        Self::Boundaries {
            indices: indices.into(),
            policy: BoundaryPolicy::Utf8Aligned,
        }
    }

    pub fn boundaries_unaligned(indices: impl Into<Vec<usize>>) -> Self {
        Self::Boundaries {
            indices: indices.into(),
            policy: BoundaryPolicy::ByteStream,
        }
    }

    pub fn for_each_chunk(&self, input: &str, mut f: impl FnMut(&[u8])) {
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
                let mut points = filter_boundaries_by_policy(input, indices, *policy);
                points.sort_unstable();
                points.dedup();
                points.retain(|&idx| idx > 0 && idx < bytes.len());
                let mut last = 0usize;
                for idx in points {
                    assert_chunk_boundary(input, last, *policy, "boundaries-start");
                    assert_chunk_boundary(input, idx, *policy, "boundaries-end");
                    if idx > last {
                        f(&bytes[last..idx]);
                    }
                    last = idx;
                }
                if last < bytes.len() {
                    assert_chunk_boundary(input, last, *policy, "boundaries-final-start");
                    assert_chunk_boundary(input, bytes.len(), *policy, "boundaries-final-end");
                    f(&bytes[last..]);
                }
            }
        }
    }
}

fn assert_chunk_boundary(input: &str, idx: usize, policy: BoundaryPolicy, context: &str) {
    if matches!(policy, BoundaryPolicy::Utf8Aligned) {
        assert!(
            input.is_char_boundary(idx),
            "chunk boundary must be UTF-8 aligned ({context}): {idx}"
        );
    }
}

pub(crate) fn filter_boundaries_by_policy(
    input: &str,
    indices: &[usize],
    policy: BoundaryPolicy,
) -> Vec<usize> {
    let len = input.len();
    let mut out = Vec::new();
    for &idx in indices {
        if idx == 0 || idx >= len {
            continue;
        }
        if matches!(policy, BoundaryPolicy::Utf8Aligned) && !input.is_char_boundary(idx) {
            continue;
        }
        out.push(idx);
    }
    out
}

pub fn default_chunk_plans() -> &'static [ChunkPlan] {
    static PLANS: std::sync::OnceLock<Vec<ChunkPlan>> = std::sync::OnceLock::new();
    PLANS.get_or_init(|| {
        let mut plans = Vec::new();
        plans.push(ChunkPlan::fixed(64));
        for size in [1usize, 2, 3, 7] {
            plans.push(ChunkPlan::fixed_unaligned(size));
        }
        plans.push(ChunkPlan::sizes_unaligned(vec![1, 1, 2, 1, 4, 8, 16, 3, 7]));
        plans.push(ChunkPlan::sizes_unaligned(vec![2, 3, 1, 5, 1, 1, 9, 2]));
        plans.push(ChunkPlan::boundaries_unaligned(vec![1, 2, 4, 5, 6, 7]));
        plans.push(ChunkPlan::boundaries(vec![3, 5]));
        plans
    })
}

pub fn deterministic_chunk_plans(input: &str) -> Vec<ChunkPlan> {
    let mut plans = Vec::new();
    for size in [1usize, 2, 3, 4, 7, 16, 64] {
        plans.push(ChunkPlan::fixed_unaligned(size));
    }
    if let Some(boundaries) = every_byte_boundaries(input, 128) {
        plans.push(ChunkPlan::boundaries_unaligned(boundaries));
    }
    let semantic_raw = semantic_boundaries(input, 256);
    if !semantic_raw.is_empty() {
        let semantic_aligned =
            filter_boundaries_by_policy(input, &semantic_raw, BoundaryPolicy::Utf8Aligned);
        if !semantic_aligned.is_empty() {
            plans.push(ChunkPlan::boundaries(semantic_aligned));
        }
        plans.push(ChunkPlan::boundaries_unaligned(semantic_raw));
    }
    plans
}
