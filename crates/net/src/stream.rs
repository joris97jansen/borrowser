use crate::{BodyProgress, NetEvent, NetworkErrorKind, limits::resource_byte_limit};
use core_types::ResourceKind;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct StreamFailure {
    pub kind: NetworkErrorKind,
    pub body: BodyProgress,
    pub error: String,
}

pub(crate) fn stream_reader<R: Read>(
    request_id: u64,
    url: &str,
    kind: ResourceKind,
    cancel_token: &Arc<AtomicBool>,
    callback: &Arc<dyn Fn(NetEvent) + Send + Sync>,
    reader: &mut R,
) -> Result<BodyProgress, StreamFailure> {
    let mut buffer = [0_u8; 32 * 1024];
    let mut total = 0_usize;
    let byte_limit = resource_byte_limit(kind);
    loop {
        let progress = || BodyProgress::try_new(kind, total).expect("bounded stream accounting");
        if cancel_token.load(Ordering::Relaxed) {
            return Err(StreamFailure {
                kind: NetworkErrorKind::Cancelled,
                body: progress(),
                error: "cancelled".into(),
            });
        }
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(progress()),
            Ok(n) => {
                let take = n.min(byte_limit - total);
                if take > 0 {
                    total += take;
                    callback(NetEvent::Chunk {
                        request_id,
                        url: url.to_string(),
                        chunk: buffer[..take].to_vec(),
                    });
                }
                if n > take {
                    return Err(StreamFailure {
                        kind: NetworkErrorKind::ResourceLimit,
                        body: BodyProgress::try_new(kind, total)
                            .expect("bounded stream accounting"),
                        error: format!(
                            "{} response exceeded byte limit of {} bytes",
                            kind.as_str(),
                            byte_limit
                        ),
                    });
                }
            }
            Err(err) => {
                return Err(StreamFailure {
                    kind: NetworkErrorKind::Read,
                    body: progress(),
                    error: format!("read error: {err}"),
                });
            }
        }
    }
}
