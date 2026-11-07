use net::{NetworkErrorKind, NetworkResponseInfo};

pub(super) fn response_summary(response: &NetworkResponseInfo, bytes_received: usize) -> String {
    let mut parts = Vec::new();

    if let Some(status_code) = response.status_code() {
        parts.push(format!("HTTP {status_code}"));
    }

    if let Some(content_type) = response.legacy_content_type() {
        parts.push(content_type.to_string());
    }

    if bytes_received > 0 {
        parts.push(format!("{bytes_received} B"));
    }

    parts.push(match response.reported_response_url() {
        Some(url) => url.to_owned(),
        None => "response URL unavailable".into(),
    });

    parts.join(" • ")
}

pub(super) fn format_network_error(
    resource_label: &str,
    url: &str,
    error_kind: NetworkErrorKind,
    status_code: Option<u16>,
    error: &str,
) -> String {
    let message = match error_kind {
        NetworkErrorKind::Cancelled => format!("Cancelled {resource_label} load: {url}"),
        NetworkErrorKind::Transport => {
            format!("Transport error loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::LocalFile => {
            format!("Local file error loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::Read => format!("Read error loading {resource_label}: {url} ({error})"),
        NetworkErrorKind::MalformedResponse => {
            format!("Malformed response loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::UnsupportedResponse => {
            format!("Unsupported response loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::ResourceLimit => {
            format!("Resource limit loading {resource_label}: {url} ({error})")
        }
    };
    match status_code {
        Some(code) => format!("{message} [HTTP {code}]"),
        None => message,
    }
}
