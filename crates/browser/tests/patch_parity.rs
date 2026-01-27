use browser::dom_store::DomStore;
use core_types::{DomHandle, DomVersion};
use html::dom_snapshot::{DomSnapshotOptions, compare_dom};
use html::golden_corpus::fixtures;
use html::{DomDiffState, DomPatch, Node, diff_dom_with_state, diff_from_empty, tokenize};
use tools::utf8::{finish_utf8, push_utf8_chunk};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundaryPolicy {
    Utf8Aligned,
    ByteStream,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ChunkPlan {
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
                for &size in sizes {
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
                    assert_chunk_boundary(input, offset, *policy, "sizes-tail");
                    f(&bytes[offset..]);
                }
            }
            ChunkPlan::Boundaries { indices, policy } => {
                let mut last = 0usize;
                for &idx in indices {
                    if idx <= last || idx > bytes.len() {
                        continue;
                    }
                    assert_chunk_boundary(input, idx, *policy, "boundary");
                    f(&bytes[last..idx]);
                    last = idx;
                }
                if last < bytes.len() {
                    f(&bytes[last..]);
                }
            }
        }
    }
}

impl std::fmt::Display for ChunkPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkPlan::Fixed { size, policy } => {
                write!(f, "fixed size={size} policy={policy:?}")
            }
            ChunkPlan::Sizes { sizes, policy } => {
                write!(
                    f,
                    "sizes count={} policy={policy:?} sizes={sizes:?}",
                    sizes.len()
                )
            }
            ChunkPlan::Boundaries { indices, policy } => {
                write!(
                    f,
                    "boundaries count={} policy={policy:?} indices={indices:?}",
                    indices.len()
                )
            }
        }
    }
}

fn assert_chunk_boundary(input: &str, idx: usize, policy: BoundaryPolicy, ctx: &str) {
    match policy {
        BoundaryPolicy::Utf8Aligned => {
            assert!(
                input.is_char_boundary(idx),
                "non-utf8 boundary idx={idx} ctx={ctx}"
            );
        }
        BoundaryPolicy::ByteStream => {}
    }
}

fn deterministic_chunk_plans(input: &str) -> Vec<ChunkPlan> {
    let mut plans = Vec::new();
    for size in [1usize, 2, 3, 4, 7, 16, 64] {
        plans.push(ChunkPlan::Fixed {
            size,
            policy: BoundaryPolicy::ByteStream,
        });
    }
    if let Some(boundaries) = every_byte_boundaries(input, 128) {
        plans.push(ChunkPlan::Boundaries {
            indices: boundaries,
            policy: BoundaryPolicy::ByteStream,
        });
    }
    let utf8_boundaries = utf8_aligned_boundaries(input, 128);
    if !utf8_boundaries.is_empty() {
        plans.push(ChunkPlan::Boundaries {
            indices: utf8_boundaries,
            policy: BoundaryPolicy::Utf8Aligned,
        });
    }
    plans
}

fn every_byte_boundaries(input: &str, max: usize) -> Option<Vec<usize>> {
    let len = input.len();
    if len <= 1 {
        return None;
    }
    let upper = len.min(max);
    let mut out = Vec::with_capacity(upper.saturating_sub(1));
    for idx in 1..upper {
        out.push(idx);
    }
    Some(out)
}

fn utf8_aligned_boundaries(input: &str, max: usize) -> Vec<usize> {
    let len = input.len();
    if len <= 1 {
        return Vec::new();
    }
    let upper = len.min(max);
    let mut out = Vec::new();
    for idx in 1..upper {
        if input.is_char_boundary(idx) {
            out.push(idx);
        }
    }
    out
}

fn fuzz_seed_count() -> usize {
    if let Ok(value) = std::env::var("BORROWSER_FUZZ_SEEDS")
        && let Ok(parsed) = value.parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    if std::env::var("CI").is_ok() { 50 } else { 200 }
}

fn fuzz_seed_base() -> u64 {
    if let Ok(value) = std::env::var("BORROWSER_FUZZ_SEED") {
        if let Ok(parsed) = u64::from_str_radix(value.trim_start_matches("0x"), 16) {
            return parsed;
        }
        if let Ok(parsed) = value.parse::<u64>() {
            return parsed;
        }
    }
    0x6c8e9cf570932bd5
}

fn derive_seed(base: u64, name: &str, salt: u64) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in name.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    base ^ hash ^ salt.wrapping_mul(0x9e3779b97f4a7c15)
}

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 32) as u32
    }

    fn gen_range(&mut self, min: usize, max: usize) -> usize {
        if max <= min {
            return min;
        }
        let span = (max - min) as u32;
        min + (self.next_u32() % span) as usize
    }
}

fn fuzz_chunk_plan(input: &str, seed: u64) -> ChunkPlan {
    let mut rng = Lcg::new(seed);
    let len = input.len().max(1);
    let aligned = rng.gen_range(0, 10) == 0;
    if aligned {
        let mut boundaries = utf8_aligned_boundaries(input, len.min(256));
        if boundaries.is_empty() {
            return ChunkPlan::Fixed {
                size: 1,
                policy: BoundaryPolicy::ByteStream,
            };
        }
        boundaries.retain(|_| rng.gen_range(0, 3) != 0);
        if boundaries.is_empty() {
            boundaries.push(len.min(1));
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        return ChunkPlan::Boundaries {
            indices: boundaries,
            policy: BoundaryPolicy::Utf8Aligned,
        };
    }
    let policy = BoundaryPolicy::ByteStream;
    match rng.gen_range(0, 3) {
        0 => {
            let size = rng.gen_range(1, len.min(32) + 1);
            ChunkPlan::Fixed { size, policy }
        }
        1 => {
            let count = rng.gen_range(1, 8);
            let mut sizes = Vec::with_capacity(count);
            for _ in 0..count {
                sizes.push(rng.gen_range(1, len.min(16) + 1));
            }
            ChunkPlan::Sizes { sizes, policy }
        }
        _ => {
            let count = rng.gen_range(1, len.min(32));
            let mut indices = Vec::with_capacity(count);
            for _ in 0..count {
                indices.push(rng.gen_range(1, len));
            }
            indices.sort_unstable();
            indices.dedup();
            ChunkPlan::Boundaries { indices, policy }
        }
    }
}

fn shrink_plan(
    _input: &str,
    plan: &ChunkPlan,
    mut fails: impl FnMut(&ChunkPlan) -> bool,
) -> ChunkPlan {
    match plan {
        ChunkPlan::Sizes { sizes, policy } => {
            let mut current = sizes.clone();
            while current.len() > 1 {
                let candidate = ChunkPlan::Sizes {
                    sizes: current[..current.len() - 1].to_vec(),
                    policy: *policy,
                };
                if fails(&candidate) {
                    current = current[..current.len() - 1].to_vec();
                } else {
                    break;
                }
            }
            ChunkPlan::Sizes {
                sizes: current,
                policy: *policy,
            }
        }
        ChunkPlan::Boundaries { indices, policy } => {
            let mut current = indices.clone();
            let mut i = 0usize;
            while i < current.len() {
                let mut candidate = current.clone();
                candidate.remove(i);
                let candidate_plan = ChunkPlan::Boundaries {
                    indices: candidate.clone(),
                    policy: *policy,
                };
                if fails(&candidate_plan) {
                    current = candidate;
                } else {
                    i += 1;
                }
            }
            ChunkPlan::Boundaries {
                indices: current,
                policy: *policy,
            }
        }
        ChunkPlan::Fixed { .. } => plan.clone(),
    }
}

fn minimize_patch_batches(
    input: &str,
    plan: &ChunkPlan,
    baseline: &Node,
    batches: &[Vec<DomPatch>],
) -> usize {
    let mut lo = 0usize;
    let mut hi = batches.len();
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if run_and_compare(input, plan, baseline, Some(mid)).is_err() {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

fn run_and_compare(
    input: &str,
    plan: &ChunkPlan,
    baseline: &Node,
    batch_limit: Option<usize>,
) -> Result<(), String> {
    let batches = patch_batches_for_plan(input, plan)?;
    let limit = batch_limit.unwrap_or(batches.len());
    let mut store = DomStore::new();
    let handle = DomHandle(1);
    store.create(handle).map_err(|e| format!("{e:?}"))?;
    let mut version = DomVersion::INITIAL;
    for batch in batches.iter().take(limit) {
        let from = version;
        let to = from.next();
        store
            .apply(handle, from, to, batch)
            .map_err(|e| format!("{e:?}"))?;
        version = to;
    }
    let actual = store.materialize(handle).map_err(|e| format!("{e:?}"))?;
    match compare_dom(
        baseline,
        &actual,
        DomSnapshotOptions {
            ignore_ids: true,
            ignore_empty_style: true,
        },
    ) {
        Ok(()) => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn patch_batches_for_plan(input: &str, plan: &ChunkPlan) -> Result<Vec<Vec<DomPatch>>, String> {
    let mut text = String::new();
    let mut carry = Vec::new();
    let mut prev_dom: Option<Node> = None;
    let mut diff_state = DomDiffState::new();
    let mut batches = Vec::new();
    let mut err: Option<String> = None;
    plan.for_each_chunk(input, |chunk| {
        if err.is_some() {
            return;
        }
        push_utf8_chunk(&mut text, &mut carry, chunk);
        let dom = build_owned_dom(&text);
        let patches = match &prev_dom {
            Some(prev) => diff_dom_with_state(prev, &dom, &mut diff_state),
            None => diff_from_empty(&dom, &mut diff_state),
        };
        match patches {
            Ok(patches) => {
                prev_dom = Some(dom);
                batches.push(patches);
            }
            Err(e) => {
                err = Some(format!("{e:?}"));
            }
        }
    });
    if let Some(message) = err {
        return Err(message);
    }
    finish_utf8(&mut text, &mut carry);
    let dom = build_owned_dom(&text);
    let patches = match &prev_dom {
        Some(prev) => diff_dom_with_state(prev, &dom, &mut diff_state),
        None => diff_from_empty(&dom, &mut diff_state),
    }
    .map_err(|e| format!("{e:?}"))?;
    batches.push(patches);
    Ok(batches)
}

fn build_owned_dom(input: &str) -> Node {
    let stream = tokenize(input);
    html::build_owned_dom(&stream)
}

fn plan_boundaries(input: &str, plan: &ChunkPlan) -> Vec<usize> {
    let bytes = input.as_bytes();
    let mut boundaries = Vec::new();
    match plan {
        ChunkPlan::Fixed { size, .. } => {
            let mut offset = 0usize;
            while offset < bytes.len() {
                let end = (offset + size).min(bytes.len());
                boundaries.push(end);
                offset = end;
            }
        }
        ChunkPlan::Sizes { sizes, .. } => {
            let mut offset = 0usize;
            for &size in sizes {
                if offset >= bytes.len() {
                    break;
                }
                let end = (offset + size).min(bytes.len());
                boundaries.push(end);
                offset = end;
            }
            if offset < bytes.len() {
                boundaries.push(bytes.len());
            }
        }
        ChunkPlan::Boundaries { indices, .. } => {
            boundaries.extend(indices.iter().copied().filter(|idx| *idx <= bytes.len()));
            if boundaries.last().copied() != Some(bytes.len()) {
                boundaries.push(bytes.len());
            }
        }
    }
    boundaries
}

fn plan_context(input: &str, plan: &ChunkPlan) -> String {
    let boundaries = plan_boundaries(input, plan);
    let last = boundaries.last().copied().unwrap_or(0);
    format!(
        "input_len={} last_boundary={} boundary_count={}",
        input.len(),
        last,
        boundaries.len()
    )
}

fn boundary_at_batch(input: &str, plan: &ChunkPlan, batch_index: usize) -> Option<usize> {
    let boundaries = plan_boundaries(input, plan);
    boundaries.get(batch_index).copied()
}

#[test]
/// Parity test treats every chunk boundary as a preview tick to maximize coverage.
fn patch_parity_corpus_deterministic() {
    for fixture in fixtures() {
        let baseline = build_owned_dom(fixture.input);
        for plan in deterministic_chunk_plans(fixture.input) {
            if let Err(err) = run_and_compare(fixture.input, &plan, &baseline, None) {
                let minimized = shrink_plan(fixture.input, &plan, |candidate| {
                    run_and_compare(fixture.input, candidate, &baseline, None).is_err()
                });
                let batches = patch_batches_for_plan(fixture.input, &minimized).unwrap_or_default();
                let min_batches =
                    minimize_patch_batches(fixture.input, &minimized, &baseline, &batches);
                panic!(
                    "patch parity failure fixture={} plan={} minimized_plan={} patches={} min_batches={} batch_boundary={:?} context={} err={}",
                    fixture.name,
                    plan,
                    minimized,
                    batches.len(),
                    min_batches,
                    boundary_at_batch(fixture.input, &minimized, min_batches.saturating_sub(1)),
                    plan_context(fixture.input, &minimized),
                    err
                );
            }
        }
    }
}

#[test]
fn patch_parity_corpus_fuzz() {
    let base = fuzz_seed_base();
    let count = fuzz_seed_count();
    for fixture in fixtures() {
        let baseline = build_owned_dom(fixture.input);
        for i in 0..count {
            let seed = derive_seed(base, fixture.name, i as u64);
            let plan = fuzz_chunk_plan(fixture.input, seed);
            if let Err(err) = run_and_compare(fixture.input, &plan, &baseline, None) {
                let minimized = shrink_plan(fixture.input, &plan, |candidate| {
                    run_and_compare(fixture.input, candidate, &baseline, None).is_err()
                });
                let batches = patch_batches_for_plan(fixture.input, &minimized).unwrap_or_default();
                let min_batches =
                    minimize_patch_batches(fixture.input, &minimized, &baseline, &batches);
                panic!(
                    "patch parity fuzz failure fixture={} seed=0x{:016x} plan={} minimized_plan={} patches={} min_batches={} batch_boundary={:?} context={} err={}",
                    fixture.name,
                    seed,
                    plan,
                    minimized,
                    batches.len(),
                    min_batches,
                    boundary_at_batch(fixture.input, &minimized, min_batches.saturating_sub(1)),
                    plan_context(fixture.input, &minimized),
                    err
                );
            }
        }
    }
}

#[test]
fn patch_parity_reset_semantics() {
    let prev = "<div><span>hi</span></div>";
    let next = "<div><em>yo</em><span>hi</span></div>";
    let baseline = build_owned_dom(next);
    let mut store = DomStore::new();
    let handle = DomHandle(1);
    store.create(handle).expect("store create failed");
    let prev_dom = build_owned_dom(prev);
    let mut diff_state = DomDiffState::new();
    let patches = diff_dom_with_state(&prev_dom, &baseline, &mut diff_state).expect("diff failed");
    assert!(
        matches!(patches.first(), Some(DomPatch::Clear)),
        "expected reset to emit Clear"
    );
    assert!(
        patches.len() > 1,
        "reset must include create stream after Clear"
    );
    let from = DomVersion::INITIAL;
    let to = from.next();
    store
        .apply(handle, from, to, &patches)
        .expect("apply failed");
    let actual = store.materialize(handle).expect("materialize failed");
    compare_dom(
        &baseline,
        &actual,
        DomSnapshotOptions {
            ignore_ids: true,
            ignore_empty_style: true,
        },
    )
    .expect("dom mismatch after reset");
}
