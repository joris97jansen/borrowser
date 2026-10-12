use super::error::Error;
use crate::{
    environment::{HEIGHT, SAMPLE, WIDTH},
    model::CanvasColor,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::io::Cursor;

const MAX_PNG: usize = 2 * 1024 * 1024;
const MAX_DECODED: usize = 8 * 1024 * 1024;

fn bad(s: impl Into<String>) -> Error {
    Error::Capture(s.into())
}

pub(super) fn sample(encoded: &str) -> Result<CanvasColor, Error> {
    if encoded.len() > MAX_PNG.div_ceil(3) * 4 {
        return Err(bad("screenshot exceeds encoded limit"));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| bad(format!("invalid PNG base64: {e}")))?;
    if bytes.len() > MAX_PNG {
        return Err(bad("screenshot exceeds PNG limit"));
    }
    validate_chunks(&bytes)?;
    let decoder =
        png::Decoder::new_with_limits(Cursor::new(&bytes), png::Limits { bytes: MAX_DECODED });
    let mut reader = decoder
        .read_info()
        .map_err(|e| bad(format!("PNG header: {e}")))?;
    let info = reader.info();
    if (info.width, info.height) != (WIDTH, HEIGHT) || info.bit_depth != png::BitDepth::Eight {
        return Err(bad("expected 640x480 RGB8/RGBA8 PNG"));
    }
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return Err(bad("unsupported PNG color type")),
    };
    let size = reader
        .output_buffer_size()
        .filter(|n| *n <= MAX_DECODED)
        .ok_or_else(|| bad("unbounded PNG output"))?;
    let mut pixels = vec![0; size];
    let output = reader
        .next_frame(&mut pixels)
        .map_err(|e| bad(format!("PNG pixels: {e}")))?;
    reader
        .finish()
        .map_err(|e| bad(format!("PNG completion: {e}")))?;
    if output.buffer_size() != WIDTH as usize * HEIGHT as usize * channels {
        return Err(bad("unexpected PNG decoded dimensions"));
    }
    let offset = (SAMPLE[1] as usize * WIDTH as usize + SAMPLE[0] as usize) * channels;
    if channels == 4 && pixels[offset + 3] != 255 {
        return Err(bad("sample is not opaque"));
    }
    Ok(CanvasColor([
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
    ]))
}

// Inspect original chunks: decoder metadata can replace conflicting color
// declarations with sRGB defaults. Never silently normalize ambiguous input.
fn validate_chunks(bytes: &[u8]) -> Result<(), Error> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(bad("invalid PNG signature"));
    }
    let mut pos = 8;
    let mut ended = false;
    let mut color_chunks = std::collections::BTreeSet::new();
    while pos < bytes.len() {
        let header = bytes
            .get(pos..pos + 8)
            .ok_or_else(|| bad("truncated PNG chunk"))?;
        let len = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = pos
            .checked_add(12)
            .and_then(|n| n.checked_add(len))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| bad("invalid PNG chunk size"))?;
        let kind = &header[4..8];
        let data = &bytes[pos + 8..end - 4];
        if matches!(kind, b"sRGB" | b"gAMA" | b"cHRM") && !color_chunks.insert(kind) {
            return Err(bad("duplicate PNG color declaration"));
        }
        match kind {
            b"IHDR" | b"IDAT" => {}
            b"IEND" => {
                if len != 0 || end != bytes.len() {
                    return Err(bad("invalid PNG end"));
                }
                ended = true;
            }
            b"sRGB" if data.len() == 1 && data[0] <= 3 => {}
            b"gAMA" if data == 45455_u32.to_be_bytes() => {}
            b"cHRM" => {
                let expected: Vec<u8> = [31270_u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000]
                    .into_iter()
                    .flat_map(u32::to_be_bytes)
                    .collect();
                if data != expected {
                    return Err(bad("non-sRGB PNG chromaticities"));
                }
            }
            _ => return Err(bad("unsupported PNG chunk/color profile")),
        }
        pos = end;
    }
    if !ended {
        return Err(bad("PNG missing IEND"));
    }
    Ok(())
}
