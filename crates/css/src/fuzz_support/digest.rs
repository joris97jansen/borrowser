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

#[cfg(test)]
thread_local! { static AJ8_LEGACY_GRAMMAR_AUDIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[cfg(test)]
fn aj8_legacy_grammar(snapshot: &str) -> String {
    let lines: Vec<_> = snapshot.split_inclusive('\n').collect();
    let mut old = String::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() == "had_invariant_violation: false" {
            continue;
        }
        assert_ne!(
            line.trim(),
            "had_invariant_violation: true",
            "cannot normalize actual integrity failure"
        );
        let changed_kind = lines.get(index + 1).is_some_and(|next| {
            matches!(
                next.trim(),
                "model-stylesheet"
                    | "model-declaration-list"
                    | "stylesheet"
                    | "declaration-list"
                    | "tokens"
            )
        });
        if line.trim() == "version: 2" && changed_kind {
            old.push_str(&line.replacen("version: 2", "version: 1", 1));
        } else {
            old.push_str(line);
        }
    }
    old
}

pub(crate) fn digest_snapshot(seed: u64, snapshots: &[String]) -> u64 {
    let mut digest = mix_u64(0, seed);

    for snapshot in snapshots {
        #[cfg(test)]
        if AJ8_LEGACY_GRAMMAR_AUDIT.with(|flag| flag.get()) {
            digest = mix_str(digest, &aj8_legacy_grammar(snapshot));
            continue;
        }
        digest = mix_str(digest, snapshot);
    }

    digest
}

#[cfg(all(test, feature = "css-fuzzing"))]
mod aj8_snapshot_audit;
