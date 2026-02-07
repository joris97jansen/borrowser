use crate::tokenizer::Tokenizer;
use crate::{Node, build_owned_dom, tokenize};
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
                // Boundaries are normalized (sorted, deduped, clipped to (0, len)).
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

pub fn run_full(input: &str) -> Node {
    let stream = tokenize(input);
    build_owned_dom(&stream)
}

pub fn run_chunked(input: &str, plan: &ChunkPlan) -> Node {
    let (dom, _) = run_chunked_with_tokens(input, plan);
    dom
}

pub fn run_chunked_with_tokens(input: &str, plan: &ChunkPlan) -> (Node, crate::TokenStream) {
    let mut tokenizer = Tokenizer::new();
    plan.for_each_chunk(input, |chunk: &[u8]| {
        tokenizer.feed(chunk);
    });
    tokenizer.finish();
    let stream = tokenizer.into_stream();
    let dom = build_owned_dom(&stream);
    (dom, stream)
}

/// Test-only helper for byte-stream parity assertions.
#[cfg(test)]
pub fn run_chunked_bytes_with_tokens(
    bytes: &[u8],
    boundaries: &[usize],
) -> (Node, crate::TokenStream) {
    let mut tokenizer = Tokenizer::new();
    let mut last = 0usize;
    for &idx in boundaries {
        assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
        tokenizer.feed(&bytes[last..idx]);
        last = idx;
    }
    if last < bytes.len() {
        tokenizer.feed(&bytes[last..]);
    }
    tokenizer.finish();
    let stream = tokenizer.into_stream();
    let dom = build_owned_dom(&stream);
    (dom, stream)
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

    let mut minimized_policy = policy;
    if !budget_exhausted && matches!(minimized_policy, BoundaryPolicy::ByteStream) {
        let aligned = filter_boundaries_by_policy(input, &boundaries, BoundaryPolicy::Utf8Aligned);
        let candidate_plan = ChunkPlan::Boundaries {
            indices: aligned.clone(),
            policy: BoundaryPolicy::Utf8Aligned,
        };
        checks += 1;
        if checks >= max_checks {
            budget_exhausted = true;
        } else if fails(&candidate_plan) {
            minimized_policy = BoundaryPolicy::Utf8Aligned;
            boundaries = aligned;
            policy_upgraded = true;
        }
    }

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

/// Materialize a DOM tree from patch batches (test-only helper).
///
/// This is intended for deterministic golden tests; it enforces patch protocol invariants
/// per batch (e.g., `Clear` must be first in a batch). `Clear` resets the arena, allowing
/// a fresh document root in subsequent batches. Returns a simplified DOM with
/// `Id::INVALID` for all nodes.
pub fn materialize_patch_batches(batches: &[Vec<crate::DomPatch>]) -> Result<Node, String> {
    use crate::dom_patch::PatchKey;
    use crate::types::Id;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[derive(Clone, Debug)]
    enum PatchKind {
        Document {
            doctype: Option<String>,
        },
        Element {
            name: Arc<str>,
            attributes: Vec<(Arc<str>, Option<String>)>,
        },
        Text {
            text: String,
        },
        Comment {
            text: String,
        },
    }

    #[derive(Clone, Debug)]
    struct PatchNode {
        kind: PatchKind,
        parent: Option<PatchKey>,
        children: Vec<PatchKey>,
    }

    #[derive(Default)]
    struct PatchArena {
        nodes: HashMap<PatchKey, PatchNode>,
        root: Option<PatchKey>,
    }

    impl PatchArena {
        fn clear(&mut self) {
            self.nodes.clear();
            self.root = None;
        }

        fn insert(&mut self, key: PatchKey, node: PatchNode) -> Result<(), String> {
            if key == PatchKey::INVALID {
                return Err("patch key must be non-zero".to_string());
            }
            if self.nodes.contains_key(&key) {
                return Err(format!("duplicate patch key {key:?}"));
            }
            self.nodes.insert(key, node);
            Ok(())
        }

        fn ensure_node(&self, key: PatchKey, context: &str) -> Result<(), String> {
            if key == PatchKey::INVALID {
                return Err(format!("invalid patch key in {context}"));
            }
            if !self.nodes.contains_key(&key) {
                return Err(format!("missing node {key:?} in {context}"));
            }
            Ok(())
        }

        fn apply_batch(&mut self, patches: &[crate::DomPatch]) -> Result<(), String> {
            if patches
                .get(1..)
                .is_some_and(|rest| rest.iter().any(|p| matches!(p, crate::DomPatch::Clear)))
            {
                return Err("Clear may only appear as the first patch in a batch".to_string());
            }
            let mut start = 0usize;
            if matches!(patches.first(), Some(crate::DomPatch::Clear)) {
                self.clear();
                start = 1;
            }
            for patch in &patches[start..] {
                match patch {
                    crate::DomPatch::Clear => {
                        return Err("Clear must be first patch in a batch".to_string());
                    }
                    crate::DomPatch::CreateDocument { key, doctype } => {
                        if self.root.is_some() {
                            return Err("document root already exists".to_string());
                        }
                        self.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Document {
                                    doctype: doctype.clone(),
                                },
                                parent: None,
                                children: Vec::new(),
                            },
                        )?;
                        self.root = Some(*key);
                    }
                    crate::DomPatch::CreateElement {
                        key,
                        name,
                        attributes,
                    } => {
                        self.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Element {
                                    name: Arc::clone(name),
                                    attributes: attributes.clone(),
                                },
                                parent: None,
                                children: Vec::new(),
                            },
                        )?;
                    }
                    crate::DomPatch::CreateText { key, text } => {
                        self.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Text { text: text.clone() },
                                parent: None,
                                children: Vec::new(),
                            },
                        )?;
                    }
                    crate::DomPatch::CreateComment { key, text } => {
                        self.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Comment { text: text.clone() },
                                parent: None,
                                children: Vec::new(),
                            },
                        )?;
                    }
                    crate::DomPatch::AppendChild { parent, child } => {
                        self.ensure_node(*parent, "AppendChild parent")?;
                        self.ensure_node(*child, "AppendChild child")?;
                        let child_parent = self.nodes.get(child).and_then(|node| node.parent);
                        if child_parent.is_some() {
                            return Err("child already has parent".to_string());
                        }
                        let parent_is_container = {
                            let parent_node = self
                                .nodes
                                .get(parent)
                                .ok_or_else(|| "missing parent".to_string())?;
                            matches!(
                                parent_node.kind,
                                PatchKind::Document { .. } | PatchKind::Element { .. }
                            )
                        };
                        if !parent_is_container {
                            return Err("AppendChild parent must be a container".to_string());
                        }
                        self.nodes
                            .get_mut(parent)
                            .ok_or_else(|| "missing parent".to_string())?
                            .children
                            .push(*child);
                        self.nodes
                            .get_mut(child)
                            .ok_or_else(|| "missing child".to_string())?
                            .parent = Some(*parent);
                    }
                    crate::DomPatch::InsertBefore {
                        parent,
                        child,
                        before,
                    } => {
                        if parent == child {
                            return Err("InsertBefore cannot attach a node to itself".to_string());
                        }
                        if child == before {
                            return Err(
                                "InsertBefore cannot insert a node before itself".to_string()
                            );
                        }
                        self.ensure_node(*parent, "InsertBefore parent")?;
                        self.ensure_node(*child, "InsertBefore child")?;
                        self.ensure_node(*before, "InsertBefore before")?;
                        let (parent_is_container, before_index) = {
                            let parent_node = self
                                .nodes
                                .get(parent)
                                .ok_or_else(|| "missing parent".to_string())?;
                            let is_container = matches!(
                                parent_node.kind,
                                PatchKind::Document { .. } | PatchKind::Element { .. }
                            );
                            let index = parent_node
                                .children
                                .iter()
                                .position(|k| k == before)
                                .ok_or_else(|| "before child not found in parent".to_string())?;
                            (is_container, index)
                        };
                        if !parent_is_container {
                            return Err("InsertBefore parent must be a container".to_string());
                        }
                        let before_parent = self
                            .nodes
                            .get(before)
                            .and_then(|node| node.parent)
                            .ok_or_else(|| "before child has no parent".to_string())?;
                        if before_parent != *parent {
                            return Err("before child is not attached to parent".to_string());
                        }
                        if self.nodes.get(child).and_then(|node| node.parent).is_some() {
                            return Err("child already has parent".to_string());
                        }
                        self.nodes
                            .get_mut(parent)
                            .ok_or_else(|| "missing parent".to_string())?
                            .children
                            .insert(before_index, *child);
                        self.nodes
                            .get_mut(child)
                            .ok_or_else(|| "missing child".to_string())?
                            .parent = Some(*parent);
                    }
                    crate::DomPatch::RemoveNode { key } => {
                        self.ensure_node(*key, "RemoveNode")?;
                        let is_root = self.root == Some(*key);
                        let is_attached =
                            self.nodes.get(key).and_then(|node| node.parent).is_some();
                        if !is_root && !is_attached {
                            return Err("RemoveNode applied to detached node".to_string());
                        }
                        self.remove_subtree(*key)?;
                    }
                    crate::DomPatch::SetAttributes { key, attributes } => {
                        self.ensure_node(*key, "SetAttributes")?;
                        let node = self
                            .nodes
                            .get_mut(key)
                            .ok_or_else(|| "missing node".to_string())?;
                        match &mut node.kind {
                            PatchKind::Element {
                                attributes: slot, ..
                            } => {
                                *slot = attributes.clone();
                            }
                            _ => return Err("SetAttributes applied to non-element".to_string()),
                        }
                    }
                    crate::DomPatch::SetText { key, text } => {
                        self.ensure_node(*key, "SetText")?;
                        let node = self
                            .nodes
                            .get_mut(key)
                            .ok_or_else(|| "missing node".to_string())?;
                        match &mut node.kind {
                            PatchKind::Text { text: slot } => {
                                *slot = text.clone();
                            }
                            _ => return Err("SetText applied to non-text".to_string()),
                        }
                    }
                }
            }
            Ok(())
        }

        fn remove_subtree(&mut self, key: PatchKey) -> Result<(), String> {
            let children = {
                let node = self
                    .nodes
                    .get(&key)
                    .ok_or_else(|| "missing node".to_string())?;
                node.children.clone()
            };
            if let Some(parent) = self.nodes.get(&key).and_then(|node| node.parent)
                && let Some(parent_node) = self.nodes.get_mut(&parent)
            {
                parent_node.children.retain(|child| *child != key);
            }
            for child in children {
                self.remove_subtree(child)?;
            }
            self.nodes.remove(&key);
            if self.root == Some(key) {
                self.root = None;
            }
            Ok(())
        }

        fn materialize(&self) -> Result<Node, String> {
            let root = self
                .root
                .ok_or_else(|| "missing document root".to_string())?;
            self.materialize_node(root)
        }

        fn materialize_node(&self, key: PatchKey) -> Result<Node, String> {
            let node = self
                .nodes
                .get(&key)
                .ok_or_else(|| "missing node".to_string())?;
            let children = node
                .children
                .iter()
                .map(|child| self.materialize_node(*child))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(match &node.kind {
                PatchKind::Document { doctype } => Node::Document {
                    id: Id::INVALID,
                    doctype: doctype.clone(),
                    children,
                },
                PatchKind::Element { name, attributes } => Node::Element {
                    id: Id::INVALID,
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                    style: Vec::new(),
                    children,
                },
                PatchKind::Text { text } => Node::Text {
                    id: Id::INVALID,
                    text: text.clone(),
                },
                PatchKind::Comment { text } => Node::Comment {
                    id: Id::INVALID,
                    text: text.clone(),
                },
            })
        }
    }

    let mut arena = PatchArena::default();
    for batch in batches {
        arena.apply_batch(batch)?;
    }
    arena.materialize()
}

/// Backwards-compatible helper: treat a single vector as one batch.
pub fn materialize_patches(patches: &[crate::DomPatch]) -> Result<Node, String> {
    materialize_patch_batches(&[patches.to_vec()])
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

fn every_byte_boundaries(input: &str, max_len: usize) -> Option<Vec<usize>> {
    let len = input.len();
    if len <= 1 || len > max_len {
        return None;
    }
    Some((1..len).collect())
}

fn semantic_boundaries(input: &str, max_points: usize) -> Vec<usize> {
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

#[cfg(test)]
mod tests {
    use super::{BoundaryPolicy, ChunkPlan, run_chunked, run_full, shrink_chunk_plan_with_stats};
    use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
    use crate::tokenizer::Tokenizer;

    #[test]
    fn chunked_fixed_matches_full() {
        let input = "<p>café &amp; crème</p>";
        let full = run_full(input);
        let chunked = run_chunked(input, &ChunkPlan::fixed_unaligned(1));
        assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
    }

    #[test]
    fn chunked_boundary_plan_allows_unaligned_splits_in_ascii_prefix() {
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
            crate::test_utils::token_snapshot(&expected),
            crate::test_utils::token_snapshot(&stream),
            "expected drained tokens to match full tokenize() snapshot"
        );
    }

    #[test]
    fn shrinker_reduces_boundary_count() {
        let input = "<p>abcd</p>";
        let plan = ChunkPlan::Boundaries {
            indices: vec![1, 2, 3, 4, 5, 6, 7],
            policy: BoundaryPolicy::ByteStream,
        };
        let (minimized, _) =
            shrink_chunk_plan_with_stats(input, &plan, |candidate| match candidate {
                ChunkPlan::Boundaries { indices, .. } => indices.len() > 2,
                _ => false,
            });
        let minimized_len = match minimized {
            ChunkPlan::Boundaries { indices, .. } => indices.len(),
            _ => 0,
        };
        assert!(
            minimized_len > 0 && minimized_len < 7,
            "expected shrinker to reduce boundary count, got {minimized_len}"
        );
    }
}
