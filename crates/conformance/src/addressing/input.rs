//! Closed AH9 input schema, shared with build-time asset validation.
//! No engine semantics or expectations are evaluated here.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AddressingCase {
    Url { rows: Vec<UrlCase> },
    Origins { rows: Vec<OriginPairCase> },
    Keys { rows: Vec<KeyPairCase> },
    Documents { rows: Vec<DocumentCase> },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UrlCase {
    pub reference: String,
    pub base: Option<String>,
    pub purpose: Option<Purpose>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Purpose {
    Document,
    Stylesheet,
    Image,
    Script,
    Font,
    Media,
    FormAction,
    Hyperlink,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OriginPairCase {
    pub left: String,
    pub right: OriginOperand,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum OriginOperand {
    Independent { url: String },
    CloneLeft {},
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeyPairCase {
    pub left: UrlCase,
    pub right: UrlCase,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentCase {
    pub html: String,
    pub requested_url: String,
    pub final_url: String,
    pub requests: Vec<ResourceRequest>,
    pub same_document_url: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceRequest {
    pub kind: RequestKind,
    pub url: String,
    pub response: Option<StylesheetResponse>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RequestKind {
    Image,
    Css,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StylesheetResponse {
    pub css: String,
    pub final_url: String,
}

impl AddressingCase {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Url { rows } => rows.len(),
            Self::Origins { rows } => rows.len(),
            Self::Keys { rows } => rows.len(),
            Self::Documents { rows } => rows.len(),
        }
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.len() == 0 {
            return Err("addressing tables must not be empty".into());
        }
        if let Self::Keys { rows } = self {
            for pair in rows {
                if pair.left.purpose.is_none() || pair.right.purpose.is_none() {
                    return Err("key operands require an explicit resource purpose".into());
                }
            }
        }
        if let Self::Documents { rows } = self {
            for row in rows {
                if row.requested_url.is_empty() {
                    return Err("document requested_url must not be empty".into());
                }
                for request in &row.requests {
                    if request.url.is_empty() {
                        return Err("resource request URL must not be empty".into());
                    }
                    if request.kind == RequestKind::Image && request.response.is_some() {
                        return Err("AH9 images must remain pending and unserved".into());
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn assets(&self) -> Vec<(&str, &str)> {
        let mut assets = Vec::new();
        if let Self::Documents { rows } = self {
            for row in rows {
                assets.push((row.html.as_str(), ".html"));
                for request in &row.requests {
                    if let Some(response) = &request.response {
                        assets.push((response.css.as_str(), ".css"));
                    }
                }
            }
        }
        assets
    }
}
