pub(crate) struct HarnessRng {
    state: u64,
}

impl HarnessRng {
    pub(crate) fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn gen_range(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        (self.next_u64() as usize) % upper
    }
}

pub(crate) fn next_chunk_len(
    remaining: usize,
    chunk_index: usize,
    max_chunk_len: usize,
    rng: &mut HarnessRng,
) -> usize {
    debug_assert!(remaining > 0);
    if chunk_index == 0 {
        return 1;
    }
    let upper = remaining.min(max_chunk_len);
    1 + rng.gen_range(upper)
}
