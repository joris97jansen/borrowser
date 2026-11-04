//! Exact values for reviewed synthetic inputs. Never a production Debug mode.
use super::input::AddressingCase;
use crate::model::HarnessError;
use ::addressing::{
    BrowserUrl, Origin, ResourceAddress, ResourceCapability, ResourceKind, ResourceOperation,
    UrlDiagnostic, UrlFailure, UrlFailureKind, UrlHost, UrlParseError,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum Availability<T> {
    Present(T),
    Unavailable(String),
    Inapplicable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "rows",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum AddressingObservation {
    Url(Vec<UrlObservation>),
    Origins(Vec<OriginPairObservation>),
    Keys(Vec<KeyPairObservation>),
    Documents(Vec<DocumentObservation>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParsedUrl {
    pub serialization: String,
    pub scheme: String,
    pub username: String,
    #[serde(deserialize_with = "required_optional")]
    pub password: Option<String>,
    #[serde(deserialize_with = "required_optional")]
    pub host: Option<HostObservation>,
    #[serde(deserialize_with = "required_optional")]
    pub port: Option<u16>,
    pub path: String,
    #[serde(deserialize_with = "required_optional")]
    pub query: Option<String>,
    #[serde(deserialize_with = "required_optional")]
    pub fragment: Option<String>,
    pub authority: bool,
    pub opaque_path: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostObservation {
    pub kind: String,
    pub value: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OriginObservation {
    pub kind: String,
    pub serialization: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddressObservation {
    pub url: String,
    pub kind: String,
    pub retrieval_key: String,
    pub retrieval_capability: String,
    pub navigation_capability: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BaseObservation {
    pub source: String,
    pub input: Availability<String>,
    pub url: Availability<ParsedUrl>,
    pub diagnostics: Vec<String>,
    pub failure: Availability<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UrlObservation {
    pub input: String,
    pub base: Availability<BaseObservation>,
    pub outcome: UrlOutcome,
    pub source: Availability<SourceObservation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum UrlOutcome {
    Parsed {
        url: Box<ParsedUrl>,
        diagnostics: Vec<String>,
        origin: OriginObservation,
        resource: Availability<AddressObservation>,
    },
    Failed {
        diagnostics: Vec<String>,
        failure: String,
    },
    UnresolvedBase {},
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OriginPairObservation {
    pub left: UrlObservation,
    pub right: UrlObservation,
    pub right_creation: String,
    pub same_origin: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeyPairObservation {
    pub left: UrlObservation,
    pub right: UrlObservation,
    pub equal_keys: bool,
    pub equal_urls: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceObservation {
    pub node: String,
    pub attribute: Availability<String>,
    pub input_kind: String,
    pub location: Availability<CssLocation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CssLocation {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentObservation {
    pub requested_url: String,
    pub final_url: String,
    pub snapshots: Vec<DocumentSnapshot>,
    pub retained_origin_same: Availability<bool>,
    pub requests: Vec<RequestObservation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentSnapshot {
    pub document_url: ParsedUrl,
    pub document_origin: OriginObservation,
    pub fallback_base: ParsedUrl,
    pub active_base: ParsedUrl,
    pub selected_base: Availability<SelectedBaseObservation>,
    pub html: Vec<HtmlObservation>,
    pub css: Vec<CssSourceObservation>,
    pub css_scope: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectedBaseObservation {
    pub source: SourceObservation,
    pub href: String,
    pub url: ParsedUrl,
    pub outcome: String,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HtmlObservation {
    pub input: String,
    pub source: SourceObservation,
    pub kind: String,
    pub base: Availability<BaseObservation>,
    pub outcome: ResourceOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ResourceOutcome {
    Resolved {
        url: Box<ParsedUrl>,
        diagnostics: Vec<String>,
        origin: OriginObservation,
        address: AddressObservation,
    },
    Failed {
        diagnostics: Vec<String>,
        failure: String,
    },
    EmptyReference {},
    Empty {},
    LocalFragment {},
    InvalidCss {},
    BlockedContext {},
    IncompleteSource {},
    UnresolvedBase {},
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CssSourceObservation {
    pub source: String,
    pub owner: String,
    pub base: BaseObservation,
    pub condition_deferred: bool,
    pub effective_participation: String,
    pub incomplete: bool,
    pub feature_notices_truncated: bool,
    pub diagnostics: Vec<CssDiagnostic>,
    pub feature_notices: Vec<CssNotice>,
    pub references: Vec<CssReferenceObservation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CssDiagnostic {
    pub kind: String,
    pub severity: String,
    pub offset: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CssNotice {
    pub context: String,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CssReferenceObservation {
    pub raw: String,
    pub decoded: Availability<String>,
    pub shape: String,
    pub contexts: Vec<String>,
    pub role: String,
    pub declaration: Availability<String>,
    pub selector: Availability<String>,
    pub source: SourceObservation,
    pub outcome: ResourceOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestObservation {
    pub kind: String,
    pub url: String,
    pub stylesheet: Availability<String>,
    pub state: String,
    pub supplied_final_url: Availability<String>,
}

// Missing component fields are malformed evidence. An explicit JSON null
// represents an absent URL component; serde's implicit Option default would
// otherwise accept a partial expectation as a complete observation.
fn required_optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

pub(crate) fn parsed(url: &BrowserUrl) -> ParsedUrl {
    ParsedUrl {
        serialization: url.as_str().into(),
        scheme: url.scheme().into(),
        username: url.username().into(),
        password: url.password().map(Into::into),
        host: url.host().map(|host| {
            let (kind, value) = match host {
                UrlHost::Domain(v) => ("domain", v.to_owned()),
                UrlHost::Opaque(v) => ("opaque", v.to_owned()),
                UrlHost::Ipv4(v) => ("ipv4", v.to_string()),
                UrlHost::Ipv6(v) => ("ipv6", v.to_string()),
            };
            HostObservation {
                kind: kind.into(),
                value,
            }
        }),
        port: url.port(),
        path: url.path().into(),
        query: url.query().map(Into::into),
        fragment: url.fragment().map(Into::into),
        authority: url.has_authority(),
        opaque_path: url.has_opaque_path(),
    }
}
pub(crate) fn origin(value: &Origin) -> OriginObservation {
    OriginObservation {
        kind: if value.is_opaque() { "opaque" } else { "tuple" }.into(),
        serialization: value.serialize(),
    }
}
pub(crate) fn address(value: &ResourceAddress) -> AddressObservation {
    fn capability(value: ResourceCapability) -> String {
        match value {
            ResourceCapability::Supported => "supported",
            ResourceCapability::Reserved => "reserved",
            ResourceCapability::Deferred => "deferred",
            ResourceCapability::Unsupported => "unsupported",
        }
        .into()
    }
    AddressObservation {
        url: value.url().as_str().into(),
        kind: kind(value.kind()).into(),
        retrieval_key: value.retrieval_key().as_str().into(),
        retrieval_capability: capability(value.capability(ResourceOperation::Retrieval)),
        navigation_capability: capability(value.capability(ResourceOperation::Navigation)),
    }
}
pub(crate) fn kind(value: ResourceKind) -> &'static str {
    match value {
        ResourceKind::Document => "document",
        ResourceKind::Stylesheet => "stylesheet",
        ResourceKind::Image => "image",
        ResourceKind::Script => "script",
        ResourceKind::Font => "font",
        ResourceKind::Media => "media",
        ResourceKind::FormAction => "form_action",
        ResourceKind::Hyperlink => "hyperlink",
        ResourceKind::Unknown => "unknown",
    }
}
pub(crate) fn diagnostics(values: &[UrlDiagnostic]) -> Vec<String> {
    values
        .iter()
        .map(|v| {
            match v {
                UrlDiagnostic::Backslash => "backslash",
                UrlDiagnostic::BoundaryControlOrSpaceIgnored => "boundary_control_or_space_ignored",
                UrlDiagnostic::DomainToAscii => "domain_to_ascii",
                UrlDiagnostic::EmbeddedCredentials => "embedded_credentials",
                UrlDiagnostic::ExpectedAuthoritySlashes => "expected_authority_slashes",
                UrlDiagnostic::ExpectedFileSlashes => "expected_file_slashes",
                UrlDiagnostic::FileHostWithDriveLetter => "file_host_with_drive_letter",
                UrlDiagnostic::NonUrlCodePoint => "non_url_code_point",
                UrlDiagnostic::NullInFragment => "null_in_fragment",
                UrlDiagnostic::InvalidPercentEncoding => "invalid_percent_encoding",
                UrlDiagnostic::TabOrNewlineIgnored => "tab_or_newline_ignored",
                UrlDiagnostic::UnencodedAtSign => "unencoded_at_sign",
                UrlDiagnostic::Unclassified => "unclassified",
            }
            .into()
        })
        .collect()
}
pub(crate) fn parse_error(value: UrlParseError) -> &'static str {
    match value {
        UrlParseError::EmptyHost => "empty_host",
        UrlParseError::InvalidDomain => "invalid_domain",
        UrlParseError::InvalidDomainCharacter => "invalid_domain_character",
        UrlParseError::InvalidPort => "invalid_port",
        UrlParseError::InvalidIpv4Address => "invalid_ipv4_address",
        UrlParseError::InvalidIpv6Address => "invalid_ipv6_address",
        UrlParseError::RelativeWithoutBase => "relative_without_base",
        UrlParseError::RelativeAgainstOpaqueBase => "relative_against_opaque_base",
    }
}
pub(crate) fn failure(value: &UrlFailure) -> Result<String, HarnessError> {
    match value.kind {
        UrlFailureKind::InvalidInput(v) => Ok(parse_error(v).into()),
        UrlFailureKind::CapacityExceeded | UrlFailureKind::BackendFailure => {
            Err(HarnessError::new(
                "addressing.implementation",
                "URL backend/capacity failure is not a fixture input verdict",
            ))
        }
    }
}
pub(crate) fn resolved(value: &ResourceAddress, events: &[UrlDiagnostic]) -> ResourceOutcome {
    ResourceOutcome::Resolved {
        url: Box::new(parsed(value.url())),
        diagnostics: diagnostics(events),
        origin: origin(&value.url().url_origin()),
        address: address(value),
    }
}
impl AddressingObservation {
    pub(crate) fn validate(&self, case: &AddressingCase) -> Result<(), HarnessError> {
        let n = match (self, case) {
            (Self::Url(rows), AddressingCase::Url { rows: inputs }) => {
                for (row, input) in rows.iter().zip(inputs) {
                    if !matches!(row.source, Availability::Inapplicable)
                        || !matches!(
                            (&row.base, input.base.is_some()),
                            (Availability::Present(_), true) | (Availability::Inapplicable, false)
                        )
                    {
                        return Err(HarnessError::new(
                            "addressing.required-data",
                            "direct URL source/base observation shape",
                        ));
                    }
                    if let UrlOutcome::Parsed { resource, .. } = &row.outcome
                        && !matches!(
                            (resource, input.purpose.is_some()),
                            (Availability::Present(_), true) | (Availability::Inapplicable, false)
                        )
                    {
                        return Err(HarnessError::new(
                            "addressing.required-data",
                            "missing/inapplicable resource observation",
                        ));
                    }
                }
                rows.len()
            }
            (Self::Origins(rows), AddressingCase::Origins { .. }) => {
                if rows.iter().any(|row| {
                    !matches!(row.left.outcome, UrlOutcome::Parsed { .. })
                        || !matches!(row.right.outcome, UrlOutcome::Parsed { .. })
                }) {
                    return Err(HarnessError::new(
                        "addressing.required-data",
                        "origin comparison requires both computed origins",
                    ));
                }
                rows.len()
            }
            (Self::Keys(rows), AddressingCase::Keys { .. }) => {
                if rows.iter().any(|row| {
                    [&row.left, &row.right].iter().any(|operand| {
                        !matches!(
                            operand.outcome,
                            UrlOutcome::Parsed {
                                resource: Availability::Present(_),
                                ..
                            }
                        )
                    })
                }) {
                    return Err(HarnessError::new(
                        "addressing.required-data",
                        "key comparison requires both resource identities",
                    ));
                }
                rows.len()
            }
            (Self::Documents(rows), AddressingCase::Documents { rows: inputs }) => {
                for (row, input) in rows.iter().zip(inputs) {
                    if row.snapshots.len()
                        != if input.same_document_url.is_some() {
                            2
                        } else {
                            1
                        }
                        || row.requests.len() != input.requests.len()
                        || !matches!(
                            (&row.retained_origin_same, input.same_document_url.is_some()),
                            (Availability::Present(_), true) | (Availability::Inapplicable, false)
                        )
                    {
                        return Err(HarnessError::new(
                            "addressing.required-data",
                            "document snapshot/request cardinality or retained-origin observation",
                        ));
                    }
                }
                for row in rows {
                    for snapshot in &row.snapshots {
                        for source in &snapshot.css {
                            for reference in &source.references {
                                if !matches!(&reference.source.location, Availability::Present(position)
                                    if position.start <= position.end && position.line > 0 && position.column > 0)
                                {
                                    return Err(HarnessError::new(
                                        "addressing.required-data",
                                        "CSS reference requires its available source span",
                                    ));
                                }
                            }
                        }
                    }
                }
                rows.len()
            }
            _ => {
                return Err(HarnessError::new(
                    "addressing.required-data",
                    "input/observation family mismatch",
                ));
            }
        };
        if n == 0 || n != case.len() {
            return Err(HarnessError::new(
                "addressing.required-data",
                "nonempty input/observation table cardinality",
            ));
        }
        Ok(())
    }
}
