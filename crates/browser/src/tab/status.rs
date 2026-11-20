use net::{NetworkErrorKind, NetworkResponseInfo};

pub(super) fn response_summary(response: &NetworkResponseInfo, bytes_received: usize) -> String {
    let mut parts = Vec::new();

    if let Some(status_code) = response.status_code() {
        parts.push(format!("HTTP {status_code}"));
    }

    if bytes_received > 0 {
        parts.push(format!("{bytes_received} B"));
    }

    parts.join(" • ")
}

pub(super) fn format_network_error(
    resource_label: &str,
    _url: &str,
    error_kind: NetworkErrorKind,
    status_code: Option<u16>,
    _error: &str,
) -> String {
    let message = match error_kind {
        NetworkErrorKind::Cancelled => format!("Cancelled {resource_label} load"),
        NetworkErrorKind::Backend => {
            format!("Backend error loading {resource_label}")
        }
        NetworkErrorKind::DnsFailure => format!("DNS error loading {resource_label}"),
        NetworkErrorKind::ConnectionFailure => format!("Connection error loading {resource_label}"),
        NetworkErrorKind::TlsFailure => format!("TLS error loading {resource_label}"),
        NetworkErrorKind::Timeout => format!("Timed out loading {resource_label}"),
        NetworkErrorKind::LocalFile => {
            format!("Local file error loading {resource_label}")
        }
        NetworkErrorKind::Read => format!("Read error loading {resource_label}"),
        NetworkErrorKind::MalformedResponse => {
            format!("Malformed response loading {resource_label}")
        }
        NetworkErrorKind::UnsupportedResponse => {
            format!("Unsupported response loading {resource_label}")
        }
        NetworkErrorKind::UnsupportedEncoding => {
            format!("Unsupported encoding loading {resource_label}")
        }
        NetworkErrorKind::ContentDecoding => {
            format!("Content decoding error loading {resource_label}")
        }
        NetworkErrorKind::ResourceLimit(_) => {
            format!("Resource limit loading {resource_label}")
        }
        NetworkErrorKind::Redirect(_) => format!("Redirect error loading {resource_label}"),
    };
    match status_code {
        Some(code) => format!("{message} [HTTP {code}]"),
        None => message,
    }
}
