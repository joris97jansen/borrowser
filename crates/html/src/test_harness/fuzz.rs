use super::chunk_plan::{BoundaryPolicy, ChunkPlan, filter_boundaries_by_policy};

pub struct FuzzChunkPlan {
    pub plan: ChunkPlan,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FuzzMode {
    Sizes,
    Boundaries,
    Semantic,
    Mixed,
}

#[derive(Clone, Copy, Debug)]
pub struct ShrinkStats {
    pub original_boundaries: usize,
    pub minimized_boundaries: usize,
    pub original_chunks: usize,
    pub minimized_chunks: usize,
    pub checks: usize,
    pub policy_upgraded: bool,
    pub budget_exhausted: bool,
}

pub fn shrink_chunk_plan_with_stats(
    input: &str,
    plan: &ChunkPlan,
    mut fails: impl FnMut(&ChunkPlan) -> bool,
) -> (ChunkPlan, ShrinkStats) {
    let policy = match plan {
        ChunkPlan::Fixed { policy, .. }
        | ChunkPlan::Sizes { policy, .. }
        | ChunkPlan::Boundaries { policy, .. } => *policy,
    };
    let len = input.len();
    let max_checks = shrink_budget();
    let mut checks = 0usize;
    let mut policy_upgraded = false;
    let mut budget_exhausted = false;
    let mut current_plan = plan.clone();

    let mut original_boundaries_vec = plan_boundaries(plan, input);
    original_boundaries_vec.sort_unstable();
    original_boundaries_vec.dedup();
    original_boundaries_vec.retain(|&idx| idx > 0 && idx < len);
    let original_boundaries = original_boundaries_vec.len();
    let original_chunks = chunk_count(len, original_boundaries);

    if let ChunkPlan::Sizes { sizes, policy } = plan {
        let mut trimmed = sizes.to_vec();
        while !trimmed.is_empty() {
            let candidate = ChunkPlan::Sizes {
                sizes: trimmed[..trimmed.len() - 1].to_vec(),
                policy: *policy,
            };
            checks += 1;
            if checks >= max_checks {
                budget_exhausted = true;
                break;
            }
            if fails(&candidate) {
                trimmed.pop();
                current_plan = candidate;
            } else {
                break;
            }
        }
    }

    let mut boundaries = plan_boundaries(&current_plan, input);
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries.retain(|&idx| idx > 0 && idx < len);

    if !boundaries.is_empty() && !budget_exhausted {
        let mut changed = true;
        while changed {
            changed = false;
            let mut i = 0usize;
            while i < boundaries.len() {
                let mut candidate = Vec::with_capacity(boundaries.len().saturating_sub(1));
                candidate.extend_from_slice(&boundaries[..i]);
                candidate.extend_from_slice(&boundaries[i + 1..]);
                let candidate_plan = ChunkPlan::Boundaries {
                    indices: candidate.clone(),
                    policy,
                };
                checks += 1;
                if checks >= max_checks {
                    budget_exhausted = true;
                    break;
                }
                if fails(&candidate_plan) {
                    boundaries = candidate;
                    changed = true;
                } else {
                    i += 1;
                }
            }
            if budget_exhausted {
                break;
            }
        }
    }

    if !boundaries.is_empty() && !budget_exhausted {
        let mut current = boundaries;
        let mut granularity = 2usize;
        loop {
            if current.is_empty() {
                break;
            }
            let n = current.len();
            let chunk = n.div_ceil(granularity);
            let mut reduced = false;
            let mut i = 0usize;
            while i < granularity {
                let start = i * chunk;
                if start >= n {
                    break;
                }
                let end = (start + chunk).min(n);
                let mut candidate = Vec::with_capacity(n - (end - start));
                candidate.extend_from_slice(&current[..start]);
                candidate.extend_from_slice(&current[end..]);
                let candidate_plan = ChunkPlan::Boundaries {
                    indices: candidate.clone(),
                    policy,
                };
                checks += 1;
                if checks >= max_checks {
                    budget_exhausted = true;
                    break;
                }
                if fails(&candidate_plan) {
                    current = candidate;
                    reduced = true;
                    if granularity > 2 {
                        granularity -= 1;
                    }
                    break;
                }
                i += 1;
            }
            if budget_exhausted {
                break;
            }
            if !reduced {
                if granularity >= n {
                    break;
                }
                granularity = (granularity * 2).min(n);
            }
        }
        boundaries = current;
    }

    if !boundaries.is_empty() && !budget_exhausted && matches!(policy, BoundaryPolicy::ByteStream) {
        let aligned = filter_boundaries_by_policy(input, &boundaries, BoundaryPolicy::Utf8Aligned);
        let candidate_plan = ChunkPlan::Boundaries {
            indices: aligned.clone(),
            policy: BoundaryPolicy::Utf8Aligned,
        };
        checks += 1;
        if checks >= max_checks {
            budget_exhausted = true;
        } else if fails(&candidate_plan) {
            boundaries = aligned;
            policy_upgraded = true;
        }
    }

    let minimized_policy = if policy_upgraded {
        BoundaryPolicy::Utf8Aligned
    } else {
        policy
    };
    let minimized_boundaries = boundaries.len();
    let minimized_chunks = chunk_count(len, minimized_boundaries);
    let minimized = ChunkPlan::Boundaries {
        indices: boundaries,
        policy: minimized_policy,
    };
    (
        minimized,
        ShrinkStats {
            original_boundaries,
            minimized_boundaries,
            original_chunks,
            minimized_chunks,
            checks,
            policy_upgraded,
            budget_exhausted,
        },
    )
}

pub fn shrink_chunk_plan(
    input: &str,
    plan: &ChunkPlan,
    fails: impl FnMut(&ChunkPlan) -> bool,
) -> ChunkPlan {
    shrink_chunk_plan_with_stats(input, plan, fails).0
}

pub fn random_chunk_plan(input: &str, seed: u64, mode: FuzzMode) -> FuzzChunkPlan {
    let mut rng = LcgRng::new(seed);
    let len = input.len();
    if len <= 1 {
        return FuzzChunkPlan {
            plan: ChunkPlan::fixed_unaligned(1),
            summary: format!("fixed_unaligned size=1 len={len} seed=0x{seed:016x}"),
        };
    }

    let semantic_raw = semantic_boundaries(input, 128);
    let semantic_mode = matches!(mode, FuzzMode::Semantic);
    let want_semantic = !semantic_raw.is_empty()
        && (semantic_mode || (matches!(mode, FuzzMode::Mixed) && rng.gen_ratio(1, 3)));
    if want_semantic {
        let indices = random_semantic_boundaries(&mut rng, &semantic_raw, len);
        if !indices.is_empty() {
            let mut policy = if rng.gen_ratio(1, 2) {
                BoundaryPolicy::Utf8Aligned
            } else {
                BoundaryPolicy::ByteStream
            };
            let plan = match policy {
                BoundaryPolicy::Utf8Aligned => {
                    let aligned = filter_boundaries_by_policy(input, &indices, policy);
                    if aligned.is_empty() {
                        policy = BoundaryPolicy::ByteStream;
                        ChunkPlan::boundaries_unaligned(indices.clone())
                    } else {
                        ChunkPlan::boundaries(aligned)
                    }
                }
                BoundaryPolicy::ByteStream => ChunkPlan::boundaries_unaligned(indices.clone()),
            };
            let summary = {
                let plan_indices = match &plan {
                    ChunkPlan::Boundaries { indices, .. } => indices.as_slice(),
                    _ => &[],
                };
                format!(
                    "semantic_boundaries policy={policy} count={} len={} seed=0x{seed:016x} boundaries={plan_indices:?}",
                    plan_indices.len(),
                    len
                )
            };
            return FuzzChunkPlan { plan, summary };
        }
    }

    if semantic_mode {
        if let Some(boundaries) = every_byte_boundaries(input, 128) {
            return FuzzChunkPlan {
                plan: ChunkPlan::boundaries_unaligned(boundaries.clone()),
                summary: format!(
                    "semantic_fallback policy=bytes count={} len={} seed=0x{seed:016x} boundaries={boundaries:?}",
                    boundaries.len(),
                    len
                ),
            };
        }
        let cap = len.min(129);
        let boundaries: Vec<usize> = (1..cap).collect();
        return FuzzChunkPlan {
            plan: ChunkPlan::boundaries_unaligned(boundaries.clone()),
            summary: format!(
                "semantic_fallback policy=bytes count={} len={} seed=0x{seed:016x} boundaries_prefix=1..{}",
                boundaries.len(),
                len,
                cap - 1
            ),
        };
    }

    let use_sizes = matches!(mode, FuzzMode::Sizes)
        || (!semantic_mode && matches!(mode, FuzzMode::Mixed) && rng.gen_ratio(2, 3));
    if use_sizes {
        let max_chunks = len.min(32);
        let chunk_count = rng.gen_range_usize(1, max_chunks + 1);
        let sizes = random_sizes(&mut rng, len, chunk_count);
        return FuzzChunkPlan {
            plan: ChunkPlan::sizes_unaligned(sizes.clone()),
            summary: format!(
                "sizes_unaligned count={} len={} seed=0x{seed:016x} sizes={sizes:?}",
                sizes.len(),
                len
            ),
        };
    }

    let max_points = len.saturating_sub(1).min(64);
    let point_count = rng.gen_range_usize(1, max_points + 1);
    let mut indices = random_boundaries(&mut rng, len, point_count);
    indices.sort_unstable();
    indices.dedup();
    let mut policy = if rng.gen_ratio(1, 4) {
        BoundaryPolicy::Utf8Aligned
    } else {
        BoundaryPolicy::ByteStream
    };
    let plan = match policy {
        BoundaryPolicy::Utf8Aligned => {
            let aligned = filter_boundaries_by_policy(input, &indices, policy);
            if aligned.is_empty() {
                policy = BoundaryPolicy::ByteStream;
                ChunkPlan::boundaries_unaligned(indices.clone())
            } else {
                ChunkPlan::boundaries(aligned)
            }
        }
        BoundaryPolicy::ByteStream => ChunkPlan::boundaries_unaligned(indices.clone()),
    };
    let summary = {
        let plan_indices = match &plan {
            ChunkPlan::Boundaries { indices, .. } => indices.as_slice(),
            _ => &[],
        };
        format!(
            "boundaries policy={policy} count={} len={} seed=0x{seed:016x} boundaries={plan_indices:?}",
            plan_indices.len(),
            len
        )
    };
    FuzzChunkPlan { plan, summary }
}

fn plan_boundaries(plan: &ChunkPlan, input: &str) -> Vec<usize> {
    let bytes = input.as_bytes();
    let mut boundaries = Vec::new();
    match plan {
        ChunkPlan::Fixed {
            size,
            policy: _policy,
        } => {
            assert!(*size > 0, "chunk size must be > 0");
            let mut offset = 0usize;
            while offset < bytes.len() {
                let end = (offset + size).min(bytes.len());
                if end < bytes.len() {
                    boundaries.push(end);
                }
                offset = end;
            }
        }
        ChunkPlan::Sizes {
            sizes,
            policy: _policy,
        } => {
            let mut offset = 0usize;
            for size in sizes {
                assert!(*size > 0, "chunk size must be > 0");
                if offset >= bytes.len() {
                    break;
                }
                let end = (offset + size).min(bytes.len());
                if end < bytes.len() {
                    boundaries.push(end);
                }
                offset = end;
            }
        }
        ChunkPlan::Boundaries { indices, policy } => {
            let mut points = filter_boundaries_by_policy(input, indices, *policy);
            points.sort_unstable();
            points.dedup();
            points.retain(|&idx| idx > 0 && idx < bytes.len());
            boundaries.extend(points);
        }
    }
    boundaries
}

fn chunk_count(len: usize, boundaries: usize) -> usize {
    if len == 0 { 0 } else { boundaries + 1 }
}

fn shrink_budget() -> usize {
    if let Ok(value) = std::env::var("BORROWSER_SHRINK_CHECKS")
        && let Ok(parsed) = value.parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    if std::env::var("CI").is_ok() {
        1_000
    } else {
        10_000
    }
}

pub(crate) fn every_byte_boundaries(input: &str, max_len: usize) -> Option<Vec<usize>> {
    let len = input.len();
    if len <= 1 || len > max_len {
        return None;
    }
    Some((1..len).collect())
}

pub(crate) fn semantic_boundaries(input: &str, max_points: usize) -> Vec<usize> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    for (idx, &byte) in bytes.iter().enumerate() {
        if matches!(
            byte,
            b'<' | b'>' | b'&' | b';' | b'"' | b'\'' | b'-' | b'/' | b'=' | b' '
        ) {
            out.push(idx);
            if idx + 1 < bytes.len() {
                out.push(idx + 1);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.len() > max_points {
        out.truncate(max_points);
    }
    out
}

fn random_sizes(rng: &mut LcgRng, len: usize, count: usize) -> Vec<usize> {
    let mut remaining = len;
    let mut sizes = Vec::with_capacity(count);
    for i in 0..count {
        if remaining == 0 {
            break;
        }
        let max_size = remaining.saturating_sub(count.saturating_sub(i + 1)).max(1);
        let biased_max = if rng.gen_ratio(7, 10) {
            max_size.clamp(1, 8)
        } else {
            max_size
        };
        let size = rng.gen_range_usize(1, biased_max + 1);
        sizes.push(size);
        remaining = remaining.saturating_sub(size);
    }
    sizes
}

fn random_boundaries(rng: &mut LcgRng, len: usize, count: usize) -> Vec<usize> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let idx = rng.gen_range_usize(1, len);
        out.push(idx);
    }
    out
}

fn random_semantic_boundaries(rng: &mut LcgRng, base: &[usize], len: usize) -> Vec<usize> {
    if base.is_empty() || len <= 1 {
        return Vec::new();
    }
    let max_points = base.len().min(32);
    let pick_count = rng.gen_range_usize(1, max_points + 1);
    let mut out = Vec::with_capacity(pick_count);
    for _ in 0..pick_count {
        let idx = base[rng.gen_index(base.len())];
        let jittered = if rng.gen_ratio(1, 2) {
            idx
        } else if rng.gen_ratio(1, 2) {
            idx.saturating_sub(1).max(1)
        } else {
            (idx + 1).min(len.saturating_sub(1))
        };
        if jittered > 0 && jittered < len {
            out.push(jittered);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

struct LcgRng {
    state: u64,
}

impl LcgRng {
    fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn gen_range_usize(&mut self, start: usize, end: usize) -> usize {
        assert!(start < end, "invalid range: {start}..{end}");
        let span = (end - start) as u64;
        (self.next_u64() % span) as usize + start
    }

    fn gen_index(&mut self, len: usize) -> usize {
        assert!(len > 0, "invalid length: {len}");
        self.gen_range_usize(0, len)
    }

    fn gen_ratio(&mut self, numerator: u32, denominator: u32) -> bool {
        assert!(denominator > 0, "invalid denominator: {denominator}");
        let roll = (self.next_u64() % denominator as u64) as u32;
        roll < numerator
    }
}
