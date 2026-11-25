//! Identical stable-API probe for the approved baseline and candidate.
//! One process executes all 18 cases; each case warms once and measures once.
use bus::{CoreCommand, CoreEvent};
use std::{
    io::Write,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
const CASE_TIMEOUT: Duration = Duration::from_secs(600);
fn once(source: &[u8], width: usize, deadline: Instant) -> u128 {
    let start = Instant::now();
    let (feed, rx) = mpsc::sync_channel(64);
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime_with_policy(
        rx,
        events,
        runtime_parse::PreviewPolicy {
            tick: Duration::ZERO,
            token_threshold: None,
            byte_threshold: Some(8192),
            patch_threshold: None,
            patch_byte_threshold: None,
        },
    );
    // Bounded producer queue, backpressure and deadline-aware forwarding. The
    // public runtime API remains unchanged; channel closure terminates it.

    thread::scope(|scope| {
        let producer = scope.spawn(move || {
            let send = |mut command| loop {
                assert!(Instant::now() < deadline, "producer deadline");
                match feed.try_send(command) {
                    Ok(()) => break,
                    Err(mpsc::TrySendError::Full(value)) => {
                        command = value;
                        thread::yield_now();
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => panic!("forwarder closed"),
                }
            };
            send(CoreCommand::ParseHtmlStart {
                tab_id: 1,
                request_id: 1,
            });
            for part in source.chunks(width) {
                send(CoreCommand::ParseHtmlChunk {
                    tab_id: 1,
                    request_id: 1,
                    bytes: part.to_vec(),
                });
            }
            send(CoreCommand::ParseHtmlDone {
                tab_id: 1,
                request_id: 1,
            });
        });
        let mut publications = 0;
        let mut terminal = false;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "case deadline");
            match received.recv_timeout(remaining.min(Duration::from_secs(1))) {
                Ok(CoreEvent::DomPatchUpdate { .. }) => {
                    assert!(!terminal, "publication after terminal");
                    publications += 1;
                    assert!(
                        publications <= source.len() / 8192 + 2,
                        "bounded publication count"
                    );
                }
                Ok(CoreEvent::HtmlParseFinished {
                    tab_id,
                    request_id,
                    result,
                    decode_report,
                    ..
                }) => {
                    assert!(!terminal, "duplicate terminal");
                    assert_eq!((tab_id, request_id), (1, 1));
                    assert!(result.is_ok(), "{result:?}");
                    assert!(decode_report.unwrap().eof_processed);
                    terminal = true;
                }
                Ok(_) => panic!("unexpected runtime event"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        producer.join().unwrap();
        assert!(terminal);
        assert!(publications > 0);
    });
    start.elapsed().as_nanos()
}
// Qualification-only SHA-256 over the actual consumed Vec. No production API
// or dependency is added; known-answer and external-corpus checks cover it.
fn sha256(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let padded_len = (input.len() + 9).div_ceil(64) * 64;
    let length = (input.len() as u64 * 8).to_be_bytes();
    for block in 0..padded_len / 64 {
        let mut bytes = [0u8; 64];
        for (i, byte) in bytes.iter_mut().enumerate() {
            let at = block * 64 + i;
            *byte = if at < input.len() {
                input[at]
            } else if at == input.len() {
                0x80
            } else if at >= padded_len - 8 {
                length[at - (padded_len - 8)]
            } else {
                0
            };
        }
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let a = w[i - 15];
            let b = w[i - 2];
            w[i] = w[i - 16]
                .wrapping_add(a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3))
                .wrapping_add(w[i - 7])
                .wrapping_add(b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10));
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut z] = h;
        for i in 0..64 {
            let t = z
                .wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25))
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let u = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22))
                .wrapping_add((a & b) ^ (a & c) ^ (b & c));
            z = g;
            g = f;
            f = e;
            e = d.wrapping_add(t);
            d = c;
            c = b;
            b = a;
            a = t.wrapping_add(u);
        }
        for (v, add) in h.iter_mut().zip([a, b, c, d, e, f, g, z]) {
            *v = v.wrapping_add(add);
        }
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}
fn main() {
    if cfg!(debug_assertions) {
        eprintln!("optimized probe required");
        std::process::exit(2);
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        8,
        "input-directory/input-manifest/warmups/samples required"
    );
    let get = |name: &str| -> &str {
        let index = args
            .iter()
            .position(|a| a == name)
            .expect("required argument");
        args.get(index + 1).expect("argument value")
    };
    assert_eq!(get("--warmups"), "1");
    assert_eq!(get("--samples"), "1");
    let directory = PathBuf::from(get("--input-directory"));
    let manifest_path = PathBuf::from(get("--input-manifest"));
    assert!(std::fs::metadata(&manifest_path).unwrap().len() <= 1024);
    let manifest = std::fs::read_to_string(manifest_path).unwrap();
    let expected: std::collections::BTreeMap<_, _> = manifest
        .lines()
        .map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].len(), 64);
            (fields[1], fields[0])
        })
        .collect();
    assert_eq!(expected.len(), 6);
    for size in [65536, 1048576] {
        for kind in ["clean", "late", "heavy"] {
            let name = format!("{size}-{kind}.html");
            let path = directory.join(&name);
            assert!(
                !std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(std::fs::metadata(&path).unwrap().len(), size as u64);
            let source = std::fs::read(path).unwrap();
            let identity = sha256(&source);
            assert_eq!(expected[name.as_str()], identity);
            for width in [1, 64, 1024] {
                println!("AJ5_CASE_START {size} {kind} {width} 1:1 {identity}");
                std::io::stdout().flush().unwrap();
                let deadline = Instant::now() + CASE_TIMEOUT;
                once(&source, width, deadline);
                let nanos = once(&source, width, deadline);
                assert_eq!(sha256(&source), identity);
                println!("AJ5_CASE_END {size} {kind} {width} {nanos} {identity}");
                std::io::stdout().flush().unwrap();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn aj5_consumed_input_sha256_known_answers() {
        assert_eq!(
            super::sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            super::sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            super::sha256(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
