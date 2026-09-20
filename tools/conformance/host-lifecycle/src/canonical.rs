//! Canonical V1 bytes are owned here, not by serde_json's serialization policy.
use crate::{Error, Result, require};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub const EVENT_BYTES: usize = 65_536;

pub fn sha256(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn digest(s: &str) -> Result<()> {
    require(
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid digest",
    )
}
pub fn text(s: &str, max: usize) -> Result<()> {
    require(
        !s.is_empty() && s.len() <= max && s.trim() == s && !s.chars().any(char::is_control),
        "invalid text",
    )
}
fn string(s: &str, out: &mut Vec<u8>) {
    out.push(b'"');
    for c in s.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\0'..='\u{1f}' => out.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes()),
            _ => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }
    out.push(b'"');
}
fn value(v: &Value, out: &mut Vec<u8>, depth: usize) -> Result<()> {
    require(depth <= 16, "encoding depth")?;
    match v {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Number(n) => out.extend_from_slice(
            n.as_u64()
                .ok_or(Error("unsigned integer required"))?
                .to_string()
                .as_bytes(),
        ),
        Value::String(s) => string(s, out),
        Value::Array(a) => {
            out.push(b'[');
            for (i, v) in a.iter().enumerate() {
                if i != 0 {
                    out.push(b',');
                }
                value(v, out, depth + 1)?;
            }
            out.push(b']');
        }
        Value::Object(m) => {
            let mut keys: Vec<_> = m.keys().collect();
            keys.sort_unstable_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            out.push(b'{');
            for (i, k) in keys.into_iter().enumerate() {
                require(k.is_ascii(), "non-ASCII schema key")?;
                if i != 0 {
                    out.push(b',');
                }
                string(k, out);
                out.push(b':');
                value(&m[k], out, depth + 1)?;
            }
            out.push(b'}');
        }
    }
    require(out.len() < EVENT_BYTES, "encoded event bound")
}
pub fn encode<T: Serialize>(input: &T) -> Result<Vec<u8>> {
    let v = serde_json::to_value(input).map_err(|_| Error("typed encoding"))?;
    let mut out = Vec::new();
    value(&v, &mut out, 0)?;
    out.push(b'\n');
    Ok(out)
}
pub fn decode<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    require(bytes.len() <= EVENT_BYTES, "event byte bound")?;
    let v: T = serde_json::from_slice(bytes).map_err(|_| Error("event schema"))?;
    require(encode(&v)? == bytes, "noncanonical event")?;
    Ok(v)
}
