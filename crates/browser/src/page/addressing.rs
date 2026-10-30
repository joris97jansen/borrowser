//! Browser-owned document lifetime and HTML frozen-base processing.
use addressing::{BrowserUrl, UrlDiagnostic, UrlFailure, UrlFailureKind, UrlParseError};
use html::PatchKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DocumentAddressState {
    document_url: BrowserUrl,
    selected_base: Option<(PatchKey, FrozenBase)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FrozenBase {
    url: BrowserUrl,
    outcome: BaseOutcome,
    diagnostics: Vec<UrlDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BaseOutcome {
    Resolved,
    InvalidHref(UrlParseError),
    ProhibitedData,
    ProhibitedJavascript,
}

impl DocumentAddressState {
    pub(crate) fn new(document_url: BrowserUrl) -> Self {
        Self {
            document_url,
            selected_base: None,
        }
    }

    pub(crate) fn document_url(&self) -> &BrowserUrl {
        &self.document_url
    }
    // UTF-8 ordinary documents only: inherited about bases and overrides are
    // unsupported creation contexts, rather than hidden alternative defaults.
    pub(crate) fn fallback_base_url(&self) -> &BrowserUrl {
        &self.document_url
    }
    pub(crate) fn active_base_url(&self) -> &BrowserUrl {
        self.selected_base
            .as_ref()
            .map_or(self.fallback_base_url(), |(_, base)| &base.url)
    }
    pub(crate) fn selected_key(&self) -> Option<PatchKey> {
        self.selected_base.as_ref().map(|(key, _)| *key)
    }
    pub(crate) fn reset_selection(&mut self) {
        self.selected_base = None;
    }
    pub(crate) fn set_document_url(&mut self, url: BrowserUrl) {
        // A selected element keeps its frozen result, even a rejected href's
        // old fallback. Updating the document URL is not a freezing trigger.
        self.document_url = url;
    }
    pub(crate) fn select(
        &mut self,
        candidate: Option<(PatchKey, String)>,
    ) -> Result<(), UrlFailure> {
        self.selected_base = candidate
            .map(|(key, href)| {
                freeze_base_href(self.fallback_base_url(), &href).map(|base| (key, base))
            })
            .transpose()?;
        Ok(())
    }
    pub(crate) fn debug_snapshot(&self) -> String {
        format!(
            "DocumentAddress {{\n    document_url: {:?},\n    fallback_base: {:?},\n    active_base: {:?},\n    selected_base: {:?},\n}}",
            self.document_url(),
            self.fallback_base_url(),
            self.active_base_url(),
            self.selected_base
        )
    }
}

fn freeze_base_href(fallback: &BrowserUrl, href: &str) -> Result<FrozenBase, UrlFailure> {
    match fallback.resolve(href) {
        Ok(parsed) => {
            let outcome = match parsed.url.scheme() {
                "data" => BaseOutcome::ProhibitedData,
                "javascript" => BaseOutcome::ProhibitedJavascript,
                _ => BaseOutcome::Resolved,
            };
            let url = if outcome == BaseOutcome::Resolved {
                parsed.url
            } else {
                fallback.clone()
            };
            Ok(FrozenBase {
                url,
                outcome,
                diagnostics: parsed.diagnostics,
            })
        }
        Err(error) => match error.kind {
            UrlFailureKind::InvalidInput(kind) => Ok(FrozenBase {
                url: fallback.clone(),
                outcome: BaseOutcome::InvalidHref(kind),
                diagnostics: error.diagnostics,
            }),
            _ => Err(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn url(input: &str) -> BrowserUrl {
        BrowserUrl::parse(input).unwrap().url
    }
    #[test]
    fn document_addressing_frozen_values_and_diagnostics() {
        let fallback = url("https://example.test/dir/page?q=1#old");
        for (href, expected, outcome) in [
            (
                "https://other.test/root/",
                "https://other.test/root/",
                BaseOutcome::Resolved,
            ),
            (
                "../assets/",
                "https://example.test/assets/",
                BaseOutcome::Resolved,
            ),
            (
                "",
                "https://example.test/dir/page?q=1",
                BaseOutcome::Resolved,
            ),
            (
                "http://[",
                fallback.as_str(),
                BaseOutcome::InvalidHref(UrlParseError::InvalidIpv6Address),
            ),
            (
                "data:text/plain,private",
                fallback.as_str(),
                BaseOutcome::ProhibitedData,
            ),
            (
                "javascript:private()",
                fallback.as_str(),
                BaseOutcome::ProhibitedJavascript,
            ),
        ] {
            let frozen = freeze_base_href(&fallback, href).unwrap();
            assert_eq!(frozen.url.as_str(), expected);
            assert_eq!(frozen.outcome, outcome);
        }
        let recovered = freeze_base_href(&fallback, " /a%zz ").unwrap();
        assert_eq!(recovered.outcome, BaseOutcome::Resolved);
        assert!(!recovered.diagnostics.is_empty());
        assert_eq!(recovered.url.as_str(), "https://example.test/a%zz");
    }
    #[test]
    fn document_addressing_fallback_changes_do_not_refreeze_selected_elements() {
        for href in ["", "http://[", "data:text/plain,x", "relative/"] {
            let mut state = DocumentAddressState::new(url("https://example.test/page#old"));
            state.select(Some((PatchKey(1), href.into()))).unwrap();
            let frozen = state.active_base_url().clone();
            state.set_document_url(url("https://example.test/page#new"));
            assert_eq!(state.fallback_base_url().fragment(), Some("new"));
            assert_eq!(state.active_base_url(), &frozen);
            state.reset_selection();
            assert_eq!(state.active_base_url(), state.document_url());
        }
    }

    #[test]
    fn document_addressing_literal_redacted_snapshot() {
        let mut state = DocumentAddressState::new(url("https://example.test/private"));
        state
            .select(Some((PatchKey(1), "http://[".into())))
            .unwrap();
        let structural = r#"BrowserUrl { scheme: "https", username: "", password: None, host_kind: Some("domain"), port: None, path: "<redacted>", query: None, fragment: None, authority: true, opaque_path: false }"#;
        assert_eq!(
            state.debug_snapshot(),
            format!(
                "DocumentAddress {{\n    document_url: {structural},\n    fallback_base: {structural},\n    active_base: {structural},\n    selected_base: Some((PatchKey(1), FrozenBase {{ url: {structural}, outcome: InvalidHref(InvalidIpv6Address), diagnostics: [] }})),\n}}"
            )
        );
    }
}
