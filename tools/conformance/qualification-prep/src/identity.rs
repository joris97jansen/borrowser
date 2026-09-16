use crate::{Error, Result, canonical, error::require};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub const AUTHORITY: &str = "candidate-only-not-qualification";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserIdentity {
    pub format: String,
    pub authority: String,
    pub product_raw: String,
    pub browser_product: String,
    pub browser_version: String,
    pub revision: Option<String>,
    pub protocol_version: String,
    pub executable_sha256: String,
    pub distribution_manifest_sha256: String,
    pub unshare_sha256: String,
    pub unshare_version: String,
}
impl BrowserIdentity {
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == "borrowser-preparation-browser-identity-v1"
                && self.authority == AUTHORITY,
            "browser record format",
        )?;
        let (p, v) = split_product(&self.product_raw)?;
        require(
            p == self.browser_product && v == self.browser_version,
            "product binding",
        )?;
        canonical::identity(&self.protocol_version)?;
        if let Some(r) = &self.revision
            && !r.is_empty()
        {
            canonical::identity(r)?;
        }
        for d in [
            &self.executable_sha256,
            &self.distribution_manifest_sha256,
            &self.unshare_sha256,
        ] {
            canonical::digest(d)?;
        }
        canonical::identity(&self.unshare_version)
    }
    pub fn build_revision(&self) -> Option<&str> {
        self.revision.as_deref().filter(|r| !r.is_empty())
    }
}
pub fn split_product(raw: &str) -> Result<(&str, &str)> {
    require(raw.matches('/').count() == 1, "ambiguous product/version")?;
    let (p, v) = raw
        .split_once('/')
        .ok_or(Error::Invalid("product/version"))?;
    canonical::identity(p)?;
    canonical::identity(v)?;
    Ok((p, v))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionTuple {
    pub product: String,
    pub revision: Option<String>,
    pub protocol: String,
}
pub fn version_result(v: &Value) -> Result<VersionTuple> {
    let o = v.as_object().ok_or(Error::Invalid("version object"))?;
    let product = o
        .get("product")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("product"))?;
    split_product(product)?;
    let protocol = o
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("protocolVersion"))?;
    canonical::identity(protocol)?;
    let revision = match o.get("revision") {
        None => None,
        Some(Value::String(s)) => {
            if !s.is_empty() {
                canonical::identity(s)?;
            }
            Some(s.clone())
        }
        _ => return Err(Error::Invalid("malformed revision")),
    };
    Ok(VersionTuple {
        product: product.into(),
        revision,
        protocol: protocol.into(),
    })
}
pub const MESSAGE_BYTES: usize = 1_048_576;
pub const TOTAL_BYTES: usize = 4_194_304;
/// Two fixed commands only. No generic method/parameter API.
pub fn version_request() -> &'static [u8] {
    b"{\"id\":1,\"method\":\"Browser.getVersion\"}\0"
}
pub fn close_request() -> &'static [u8] {
    b"{\"id\":2,\"method\":\"Browser.close\"}\0"
}
#[derive(Default)]
pub struct Framer {
    pending: Vec<u8>,
    total: usize,
}
impl Framer {
    /// Reserve both fixed outgoing requests within the aggregate budget.
    pub fn for_probe() -> Self {
        Self {
            pending: Vec::new(),
            total: version_request().len() + close_request().len(),
        }
    }
    pub fn finish(&self) -> Result<()> {
        require(self.pending.is_empty(), "truncated protocol frame")
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Value>> {
        self.total = self
            .total
            .checked_add(bytes.len())
            .ok_or(Error::Invalid("protocol overflow"))?;
        require(self.total <= TOTAL_BYTES, "protocol total")?;
        let mut frames = Vec::new();
        for &b in bytes {
            if b == 0 {
                require(!self.pending.is_empty(), "empty protocol message")?;
                frames.push(
                    serde_json::from_slice::<Unique>(&self.pending)
                        .map_err(|_| Error::Invalid("protocol JSON"))?
                        .0,
                );
                self.pending.clear();
            } else {
                require(self.pending.len() < MESSAGE_BYTES, "protocol message limit")?;
                self.pending.push(b);
            }
        }
        Ok(frames)
    }
}
pub fn response(v: &Value, id: u64) -> Result<&Value> {
    let o = v.as_object().ok_or(Error::Invalid("response object"))?;
    require(
        o.get("id").and_then(Value::as_u64) == Some(id)
            && !o.contains_key("error")
            && !o.contains_key("sessionId")
            && !o.contains_key("method"),
        "unexpected protocol response",
    )?;
    o.get("result").ok_or(Error::Invalid("missing result"))
}
/// Reject duplicate object keys instead of accepting serde_json::Value's last value.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bounded JSON with unique keys")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Unique(Value::Number(n)))
                    .ok_or(E::custom("number"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                use serde::de::Error;
                let mut out = Vec::new();
                while let Some(Unique(v)) = a.next_element()? {
                    if out.len() == 4096 {
                        return Err(A::Error::custom("JSON array limit"));
                    }
                    out.push(v);
                }
                Ok(Unique(Value::Array(out)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                use serde::de::Error;
                let mut out = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if out.len() == 128 || out.contains_key(&k) {
                        return Err(A::Error::custom("duplicate key/object limit"));
                    }
                    let Unique(v) = a.next_value()?;
                    out.insert(k, v);
                }
                Ok(Unique(Value::Object(out)))
            }
        }
        d.deserialize_any(V)
    }
}
