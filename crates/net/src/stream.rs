use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use core_types::{NetworkErrorKind, ResourceKind};

use crate::{NetEvent, limits::resource_byte_limit};

pub(crate) fn stream_reader<R: Read>(
    request_id: u64,
    url: &str,
    kind: ResourceKind,
    cancel_token: &Arc<AtomicBool>,
    callback: &Arc<dyn Fn(NetEvent) + Send + Sync>,
    reader: &mut R,
) -> Result<usize, (NetworkErrorKind, String)> {
    let mut buffer = [0_u8; 32 * 1024];
    let mut total = 0_usize;
    let byte_limit = resource_byte_limit(kind);

    // Resource-limit policy is streaming-by-default: bytes up to the configured
    // cap may already have been delivered before the over-limit read is
    // observed. Callers must treat `ResourceLimit` as terminal failure and
    // discard partial state where a complete resource is required. In
    // Borrowser that means images clear buffered bytes, CSS aborts buffered
    // parser state, and HTML never receives a terminal `Done` event.
    loop {
        if cancel_token.load(Ordering::Relaxed) {
            return Err((NetworkErrorKind::Cancelled, "cancelled".into()));
        }

        match reader.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(n) => {
                let remaining = byte_limit.saturating_sub(total);
                let take = n.min(remaining);
                if take > 0 {
                    total += take;
                    callback(NetEvent::Chunk {
                        request_id,
                        url: url.to_string(),
                        chunk: buffer[..take].to_vec(),
                    });
                }

                if n > take {
                    return Err((
                        NetworkErrorKind::ResourceLimit,
                        format!(
                            "{} response exceeded byte limit of {} bytes",
                            kind.as_str(),
                            byte_limit
                        ),
                    ));
                }
            }
            Err(err) => return Err((NetworkErrorKind::Read, format!("read error: {err}"))),
        }
    }
}
