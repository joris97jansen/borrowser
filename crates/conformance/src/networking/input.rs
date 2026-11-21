//! Closed AI9 data: each row is one Fetch and a finite immutable route table.
use super::{bounds, purpose::Purpose};
use serde::{Deserialize, Serialize};

// Stop before accumulating excess elements. Byte input is bounded before parsing.
pub(super) fn sequence<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Visitor<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Visitor<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "at most {N} elements")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut a: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while out.len() < N {
                match a.next_element()? {
                    Some(v) => out.push(v),
                    None => return Ok(out),
                }
            }
            if a.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom(
                    "networking sequence budget exceeded",
                ));
            }
            Ok(out)
        }
    }
    d.deserialize_seq(Visitor::<T, N>(std::marker::PhantomData))
}
macro_rules! sequences {
    ($($name:ident : $ty:ty, $limit:expr;)*) => {$ (
        fn $name<'de,D:serde::Deserializer<'de>>(d:D)->Result<Vec<$ty>,D::Error>{sequence::<D,$ty,$limit>(d)}
    )*};
}
sequences! { rows: FetchRow,32; routes: Registration,32; headers: Header,64; bytes: u8,4096; header_bytes:u8,8192; }

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum NetworkingCase {
    Fetch {
        #[serde(deserialize_with = "rows")]
        rows: Vec<FetchRow>,
    },
    Deferred {
        feature: DeferredFeature,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeferredFeature {
    JavascriptFetch,
    HttpCache,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FetchRow {
    pub request: RequestInput,
    #[serde(deserialize_with = "routes")]
    pub routes: Vec<Registration>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestInput {
    pub url: String,
    pub purpose: Purpose,
    pub route: Route,
    pub method: String,
    #[serde(deserialize_with = "headers")]
    pub headers: Vec<Header>,
    pub initiator_url: Option<String>,
    pub requesting_origin_url: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Route {
    Html,
    Css,
    Image,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Header(
    pub String,
    #[serde(deserialize_with = "header_bytes")] pub Vec<u8>,
);
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Registration {
    pub url: String,
    pub method: String,
    pub outcome: RouteOutcome,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RouteOutcome {
    Response {
        status: u16,
        #[serde(deserialize_with = "headers")]
        headers: Vec<Header>,
        body: BodyInput,
        reported_response_url: Option<String>,
        read_failure_after: Option<usize>,
    },
    NetworkError {
        error: AcquisitionFailure,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AcquisitionFailure {
    DnsFailure,
    ConnectionFailure,
    TlsFailure,
    Timeout,
    Backend,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BodyInput {
    Bytes {
        #[serde(deserialize_with = "bytes")]
        bytes: Vec<u8>,
    },
    Repeat {
        byte: u8,
        len: usize,
    },
}
impl BodyInput {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Bytes { bytes } => bytes.len(),
            Self::Repeat { len, .. } => *len,
        }
    }
    pub(crate) fn materialize(&self) -> Vec<u8> {
        match self {
            Self::Bytes { bytes } => bytes.clone(),
            Self::Repeat { byte, len } => vec![*byte; *len],
        }
    }
}
impl NetworkingCase {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let Self::Fetch { rows } = self else {
            return Ok(());
        };
        if rows.is_empty() || rows.len() > 32 {
            return Err("networking requires 1..32 rows".into());
        }
        fn url(s: &str) -> Result<(), String> {
            if s.len() > 4096 {
                Err("networking URL budget exceeded".into())
            } else {
                Ok(())
            }
        }
        for row in rows {
            url(&row.request.url)?;
            for s in [
                &row.request.initiator_url,
                &row.request.requesting_origin_url,
            ]
            .into_iter()
            .flatten()
            {
                url(s)?;
            }
            if row.request.method.len() > 32 {
                return Err("method token budget exceeded".into());
            }
            let mut total = 0usize;
            for route in &row.routes {
                url(&route.url)?;
                if route.method != "GET" {
                    return Err("registered route requires GET".into());
                }
                if let RouteOutcome::Response {
                    body,
                    reported_response_url,
                    read_failure_after,
                    ..
                } = &route.outcome
                {
                    if let Some(u) = reported_response_url {
                        url(u)?;
                    }
                    if body.len() > bounds::BODY {
                        return Err("fixture body budget exceeded".into());
                    }
                    total = total
                        .checked_add(body.len())
                        .ok_or("fixture body overflow")?;
                    if total > bounds::ROW_BODY {
                        return Err("row body budget exceeded".into());
                    }
                    if read_failure_after.is_some_and(|n| n > body.len()) {
                        return Err("body failure offset exceeds fixture body".into());
                    }
                }
            }
        }
        Ok(())
    }
}
