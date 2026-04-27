pub(crate) fn mix_u64(state: u64, value: u64) -> u64 {
    let mut next = state ^ value;
    next = next.wrapping_mul(0x100000001b3);
    next.rotate_left(7)
}

pub(crate) fn mix_usize(state: u64, value: usize) -> u64 {
    mix_u64(state, value as u64)
}

pub(crate) fn mix_bytes(mut state: u64, bytes: &[u8]) -> u64 {
    state = mix_usize(state, bytes.len());

    for &byte in bytes {
        state = mix_u64(state, u64::from(byte));
    }

    state
}

pub(crate) fn mix_str(state: u64, value: &str) -> u64 {
    mix_bytes(state, value.as_bytes())
}

pub(crate) fn digest_snapshot(seed: u64, snapshots: &[String]) -> u64 {
    let mut digest = mix_u64(0, seed);

    for snapshot in snapshots {
        digest = mix_str(digest, snapshot);
    }

    digest
}
