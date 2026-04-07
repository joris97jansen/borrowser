use core_types::{NetworkErrorKind, NetworkResponseInfo};

pub(super) fn response_summary(response: &NetworkResponseInfo, bytes_received: usize) -> String {
    let mut parts = Vec::new();

    if let Some(status_code) = response.status_code {
        parts.push(format!("HTTP {status_code}"));
    }

    if let Some(content_type) = response.content_type.as_deref() {
        parts.push(content_type.to_string());
    }

    if bytes_received > 0 {
        parts.push(format!("{bytes_received} B"));
    }

    if response.was_redirected() {
        parts.push(format!("final {}", response.final_url));
    } else {
        parts.push(response.display_url().to_string());
    }

    parts.join(" • ")
}

pub(super) fn format_network_error(
    resource_label: &str,
    url: &str,
    error_kind: NetworkErrorKind,
    status_code: Option<u16>,
    error: &str,
) -> String {
    match error_kind {
        NetworkErrorKind::Cancelled => format!("Cancelled {resource_label} load: {url}"),
        NetworkErrorKind::HttpStatus => match status_code {
            Some(status_code) => {
                format!("HTTP {status_code} while loading {resource_label}: {url}")
            }
            None => format!("HTTP error while loading {resource_label}: {url}"),
        },
        NetworkErrorKind::Transport => {
            format!("Transport error loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::LocalFile => {
            format!("Local file error loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::Read => format!("Read error loading {resource_label}: {url} ({error})"),
        NetworkErrorKind::ResourceLimit => {
            format!("Resource limit loading {resource_label}: {url} ({error})")
        }
    }
}
